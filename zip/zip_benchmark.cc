#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

#include "converters.h"
#include "file.h"
#include "read.h"
#include "write.h"
#include "absl/status/statusor.h"
#include "absl/strings/str_cat.h"
#include "absl/strings/string_view.h"
#include "third_party/benchmark/include/benchmark/benchmark.h"

namespace security::zip {
namespace {

std::string GenerateTestData(size_t size) {
  std::string data;
  data.reserve(size);
  for (size_t i = 0; i < size; ++i) {
    data.push_back(static_cast<char>((i * 73 + 19) % 256));
  }
  return data;
}

std::string BuildTestZip(size_t file_count, size_t file_size,
                         CompressionMethod method) {
  auto writer = ZipWriter::FromBuffer("", /*append=*/false).value();
  ZipWriterFileOptions options;
  (void)options.SetCompressionMethod(method);
  std::string payload = GenerateTestData(file_size);
  for (size_t i = 0; i < file_count; ++i) {
    std::string name = absl::StrCat("entry_", i, ".bin");
    (void)writer.StartFile(name, options);
    (void)writer.WriteData(payload);
  }
  auto res = writer.Finish().value();
  return std::string(reinterpret_cast<const char*>(res.data()), res.size());
}

// Benchmark streaming write throughput.
void BM_StreamWriter_Write(benchmark::State& state) {
  const size_t file_count = static_cast<size_t>(state.range(0));
  const size_t file_size = static_cast<size_t>(state.range(1));
  const std::string payload = GenerateTestData(file_size);

  ZipWriterFileOptions options;
  (void)options.SetCompressionMethod(CompressionMethod::kStored);

  for (auto _ : state) {
    auto writer = ZipStreamWriter::FromBuffer().value();
    for (size_t i = 0; i < file_count; ++i) {
      std::string name = absl::StrCat("stream_file_", i, ".bin");
      (void)writer.StartFile(name, options);
      (void)writer.WriteData(payload);
    }
    auto result = writer.Finish();
    benchmark::DoNotOptimize(result);
  }

  state.SetBytesProcessed(state.iterations() * file_count * file_size);
}
BENCHMARK(BM_StreamWriter_Write)
    ->Args({1, 1024})
    ->Args({10, 1024})
    ->Args({1, 65536})
    ->Args({10, 65536});

// Benchmark seekable write throughput.
void BM_SeekableWriter_Write(benchmark::State& state) {
  const size_t file_count = static_cast<size_t>(state.range(0));
  const size_t file_size = static_cast<size_t>(state.range(1));
  const std::string payload = GenerateTestData(file_size);

  ZipWriterFileOptions options;
  (void)options.SetCompressionMethod(CompressionMethod::kStored);

  for (auto _ : state) {
    auto writer = ZipWriter::FromBuffer("", /*append=*/false).value();
    for (size_t i = 0; i < file_count; ++i) {
      std::string name = absl::StrCat("seek_file_", i, ".bin");
      (void)writer.StartFile(name, options);
      (void)writer.WriteData(payload);
    }
    auto result = writer.Finish();
    benchmark::DoNotOptimize(result);
  }

  state.SetBytesProcessed(state.iterations() * file_count * file_size);
}
BENCHMARK(BM_SeekableWriter_Write)
    ->Args({1, 1024})
    ->Args({10, 1024})
    ->Args({1, 65536})
    ->Args({10, 65536});

// Benchmark streaming write with Deflate compression.
void BM_StreamWriter_Deflated(benchmark::State& state) {
  const size_t file_size = static_cast<size_t>(state.range(0));
  const std::string payload = GenerateTestData(file_size);

  ZipWriterFileOptions options;
  (void)options.SetCompressionMethod(CompressionMethod::kDeflated);

  for (auto _ : state) {
    auto writer = ZipStreamWriter::FromBuffer().value();
    (void)writer.StartFile("deflated.bin", options);
    (void)writer.WriteData(payload);
    auto result = writer.Finish();
    benchmark::DoNotOptimize(result);
  }

  state.SetBytesProcessed(state.iterations() * file_size);
}
BENCHMARK(BM_StreamWriter_Deflated)->Args({1024})->Args({65536});

// Benchmark streaming read throughput.
void BM_StreamReader_SequentialRead(benchmark::State& state) {
  const size_t file_count = static_cast<size_t>(state.range(0));
  const size_t file_size = static_cast<size_t>(state.range(1));
  const std::string zip_data =
      BuildTestZip(file_count, file_size, CompressionMethod::kStored);

  for (auto _ : state) {
    auto reader = ZipStreamReader::FromBuffer(zip_data).value();
    for (size_t i = 0; i < file_count; ++i) {
      auto file_opt = reader.ReadNextFile().value();
      if (!file_opt.has_value()) break;
      auto content = file_opt->GetFileData().value();
      benchmark::DoNotOptimize(content);
    }
  }

  state.SetBytesProcessed(state.iterations() * file_count * file_size);
}
BENCHMARK(BM_StreamReader_SequentialRead)
    ->Args({1, 1024})
    ->Args({10, 1024})
    ->Args({1, 65536})
    ->Args({10, 65536});

// Benchmark seekable archive read throughput.
void BM_ArchiveReader_GetFileByIndex(benchmark::State& state) {
  const size_t file_count = static_cast<size_t>(state.range(0));
  const size_t file_size = static_cast<size_t>(state.range(1));
  const std::string zip_data =
      BuildTestZip(file_count, file_size, CompressionMethod::kStored);

  for (auto _ : state) {
    auto archive = ZipArchive::FromBuffer(zip_data).value();
    for (size_t i = 0; i < file_count; ++i) {
      auto file = archive.GetFileByIndex(i).value();
      auto content = file.GetFileData().value();
      benchmark::DoNotOptimize(content);
    }
  }

  state.SetBytesProcessed(state.iterations() * file_count * file_size);
}
BENCHMARK(BM_ArchiveReader_GetFileByIndex)
    ->Args({1, 1024})
    ->Args({10, 1024})
    ->Args({1, 65536})
    ->Args({10, 65536});

// Benchmark streaming chunked reading.
void BM_StreamReader_ChunkedRead(benchmark::State& state) {
  const size_t chunk_size = static_cast<size_t>(state.range(0));
  constexpr size_t kFileSize = 65536;
  const std::string zip_data =
      BuildTestZip(1, kFileSize, CompressionMethod::kStored);

  for (auto _ : state) {
    auto reader = ZipStreamReader::FromBuffer(zip_data).value();
    auto file_opt = reader.ReadNextFile().value();
    if (file_opt.has_value()) {
      size_t total_read = 0;
      while (total_read < kFileSize) {
        auto chunk = file_opt->ReadBytes(chunk_size).value();
        if (chunk.empty()) break;
        total_read += chunk.size();
        benchmark::DoNotOptimize(chunk);
      }
    }
  }

  state.SetBytesProcessed(state.iterations() * kFileSize);
}
BENCHMARK(BM_StreamReader_ChunkedRead)
    ->Args({1024})
    ->Args({4096})
    ->Args({16384});

}  // namespace
}  // namespace security::zip
