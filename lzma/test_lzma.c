#include <stdint.h>
#include <stdio.h>

#include "third_party/liblzma/lzma.h"

int main() {
  lzma_stream strm = LZMA_STREAM_INIT;
  lzma_ret ret = lzma_stream_decoder(
      &strm, UINT64_MAX, LZMA_CONCATENATED);  // NOLINT(misc-include-cleaner)
  printf("init ret: %d\n", ret);

  const uint8_t data[] =
      "\xFD\x37\x7A\x58\x5A\x00\x00\x00\xFF\x12\xD9\x41\x00\x00\x01\x01\x00";
  strm.next_in = data;
  strm.avail_in = sizeof(data) - 1;  // 17 bytes
  uint8_t out[1024];
  strm.next_out = out;
  strm.avail_out = sizeof(out);

  ret = lzma_code(&strm, LZMA_FINISH);
  printf("code ret: %d\n", ret);

  return 0;
}
