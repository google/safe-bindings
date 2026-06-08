#include <iostream>
#include <string>
#include <utility>

#include "read.h"
#include "write.h"

int main(int argc, char** argv) {
  std::string file_name = "test.txt";
  std::string file_data = "Hello, Zip wrapper example!";

  auto writer_or = security::zip::ZipWriter::FromBuffer("", false);
  if (!writer_or.ok()) {
    std::cerr << "Failed to create writer: " << writer_or.status() << std::endl;
    return 1;
  }
  auto& writer = *writer_or;

  security::zip::ZipWriterFileOptions options;
  if (auto status = writer.StartFile(file_name, options); !status.ok()) {
    std::cerr << "Failed to start file: " << status << std::endl;
    return 1;
  }

  if (auto status = writer.WriteData(file_data); !status.ok()) {
    std::cerr << "Failed to write data: " << status << std::endl;
    return 1;
  }

  auto finished_data_or = writer.Finish();
  if (!finished_data_or.ok()) {
    std::cerr << "Failed to finish writer: " << finished_data_or.status()
              << std::endl;
    return 1;
  }
  auto finished_data = std::move(finished_data_or).value();

  std::cout << "Successfully created zip in memory, size: "
            << finished_data.AsStringView().size() << " bytes" << std::endl;

  auto archive_or =
      security::zip::ZipArchive::FromBuffer(finished_data.AsStringView());
  if (!archive_or.ok()) {
    std::cerr << "Failed to open archive from buffer: " << archive_or.status()
              << std::endl;
    return 1;
  }
  auto& archive = *archive_or;

  auto num_files_or = archive.GetLength();
  if (!num_files_or.ok()) {
    std::cerr << "Failed to get length: " << num_files_or.status() << std::endl;
    return 1;
  }
  std::cout << "Number of files in archive: " << *num_files_or << std::endl;

  auto file_or = archive.GetFileByIndex(0);
  if (!file_or.ok()) {
    std::cerr << "Failed to get file by index: " << file_or.status()
              << std::endl;
    return 1;
  }
  auto& file = *file_or;

  auto read_name_or = file.GetFileName();
  auto read_data_or = file.GetFileData();

  if (read_name_or.ok() && read_data_or.ok()) {
    std::cout << "File in zip: " << *read_name_or << std::endl;
    std::cout << "Content: " << read_data_or->AsStringView() << std::endl;
  }

  return 0;
}
