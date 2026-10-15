#include <cstddef>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "converters.h"
#include "file.h"
#include "read.h"
#include "write.h"
#include "absl/log/check.h"
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
  absl::StatusOr<ZipWriter> writer_or =
      ZipWriter::FromBuffer("", /*append=*/false);
  CHECK_OK(writer_or.status());
  ZipWriter writer = *std::move(writer_or);
  ZipWriterFileOptions options;
  (void)options.SetCompressionMethod(method);
  std::string payload = GenerateTestData(file_size);
  for (size_t i = 0; i < file_count; ++i) {
    std::string name = absl::StrCat("entry_", i, ".bin");
    (void)writer.StartFile(name, options);
    (void)writer.WriteData(payload);
  }
  absl::StatusOr<RustVecU8Wrapper> res_or = writer.Finish();
  CHECK_OK(res_or.status());
  RustVecU8Wrapper res = *std::move(res_or);
  return std::string(res.data(), res.size());
}

// Benchmark streaming read throughput.
void BM_StreamReader_SequentialRead(benchmark::State& state) {
  const size_t file_count = static_cast<size_t>(state.range(0));
  const size_t file_size = static_cast<size_t>(state.range(1));
  const std::string zip_data =
      BuildTestZip(file_count, file_size, CompressionMethod::kStored);

  for (auto _ : state) {
    absl::StatusOr<ZipStreamReader> reader_or =
        ZipStreamReader::FromBuffer(zip_data);
    CHECK_OK(reader_or.status());
    ZipStreamReader reader = *std::move(reader_or);
    for (size_t i = 0; i < file_count; ++i) {
      absl::StatusOr<std::optional<ZipFile>> file_opt_or =
          reader.ReadNextFile();
      CHECK_OK(file_opt_or.status());
      std::optional<ZipFile> file_opt = *std::move(file_opt_or);
      if (!file_opt.has_value()) break;
      absl::StatusOr<RustVecU8Wrapper> content_or = file_opt->GetFileData();
      CHECK_OK(content_or.status());
      RustVecU8Wrapper content = *std::move(content_or);
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
    absl::StatusOr<ZipArchive> archive_or = ZipArchive::FromBuffer(zip_data);
    CHECK_OK(archive_or.status());
    ZipArchive archive = *std::move(archive_or);
    for (size_t i = 0; i < file_count; ++i) {
      absl::StatusOr<ZipFile> file_or = archive.GetFileByIndex(i);
      CHECK_OK(file_or.status());
      ZipFile file = *std::move(file_or);
      absl::StatusOr<RustVecU8Wrapper> content_or = file.GetFileData();
      CHECK_OK(content_or.status());
      RustVecU8Wrapper content = *std::move(content_or);
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
    absl::StatusOr<ZipStreamReader> reader_or =
        ZipStreamReader::FromBuffer(zip_data);
    CHECK_OK(reader_or.status());
    ZipStreamReader reader = *std::move(reader_or);
    absl::StatusOr<std::optional<ZipFile>> file_opt_or = reader.ReadNextFile();
    CHECK_OK(file_opt_or.status());
    std::optional<ZipFile> file_opt = *std::move(file_opt_or);
    if (file_opt.has_value()) {
      size_t total_read = 0;
      while (total_read < kFileSize) {
        absl::StatusOr<RustVecU8Wrapper> chunk_or =
            file_opt->ReadBytes(chunk_size);
        CHECK_OK(chunk_or.status());
        RustVecU8Wrapper chunk = *std::move(chunk_or);
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
