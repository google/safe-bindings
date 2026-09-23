#include <cmath>
#include <complex>
#include <iomanip>
#include <iostream>
#include <numbers>
#include <string_view>
#include <vector>

#include "fft.h"
#include "absl/status/status.h"
#include "absl/status/status_macros.h"
#include "absl/types/span.h"

namespace {

void PrintComplex(std::string_view label,
                  absl::Span<const std::complex<double>> values) {
  std::cout << label << std::fixed << std::setprecision(2);
  for (const auto& v : values) {
    std::cout << "(" << v.real() << (v.imag() >= 0 ? "+" : "") << v.imag()
              << "i) ";
  }
  std::cout << "\n";
}

void PrintReal(std::string_view label, absl::Span<const double> values) {
  std::cout << label << std::fixed << std::setprecision(2);
  for (double v : values) {
    std::cout << v << " ";
  }
  std::cout << "\n";
}

absl::Status RunComplexFftRoundTrip() {
  std::vector<std::complex<double>> input = {
      {1.0, 0.0}, {2.0, -1.0}, {0.0, -1.0}, {-1.0, 2.0}};
  std::vector<std::complex<double>> spectrum(input.size());
  std::vector<std::complex<double>> recovered(input.size());

  security::fft::FftPlan<double, 1> plan({input.size()});
  ABSL_RETURN_IF_ERROR(security::fft::Fft(absl::MakeConstSpan(input),
                                     absl::MakeSpan(spectrum), plan));
  ABSL_RETURN_IF_ERROR(security::fft::Ifft(absl::MakeConstSpan(spectrum),
                                      absl::MakeSpan(recovered), plan));

  PrintComplex("1D FFT input:     ", input);
  PrintComplex("1D FFT output:    ", spectrum);
  PrintComplex("1D IFFT output:   ", recovered);
  return absl::OkStatus();
}

absl::Status RunLowPassFilterR2c() {
  constexpr size_t kNumSamples = 8;
  std::vector<double> noisy_signal(kNumSamples);
  for (size_t i = 0; i < kNumSamples; ++i) {
    double t = static_cast<double>(i) / kNumSamples;
    noisy_signal[i] = 2.0 * std::cos(2.0 * std::numbers::pi * 1.0 * t) +
                      0.5 * std::cos(2.0 * std::numbers::pi * 3.0 * t);
  }

  std::vector<std::complex<double>> spectrum(kNumSamples / 2 + 1);
  security::fft::R2cFftPlan<double, 1> plan({kNumSamples});
  ABSL_RETURN_IF_ERROR(security::fft::FftR2c(absl::MakeConstSpan(noisy_signal),
                                        absl::MakeSpan(spectrum), plan));

  for (size_t bin = 2; bin < spectrum.size(); ++bin) {
    spectrum[bin] = {0.0, 0.0};
  }

  std::vector<double> filtered_signal(kNumSamples);
  ABSL_RETURN_IF_ERROR(security::fft::IfftR2c(
      absl::MakeConstSpan(spectrum), absl::MakeSpan(filtered_signal), plan));

  std::cout << "\n";
  PrintReal("Noisy signal:     ", noisy_signal);
  PrintReal("Filtered signal:  ", filtered_signal);
  return absl::OkStatus();
}

absl::Status Run2dDctRoundTrip() {
  std::vector<double> block = {10.0, 20.0, 30.0, 40.0, 50.0, 60.0};
  std::vector<double> coeffs(block.size());
  std::vector<double> recovered(block.size());

  security::fft::DctPlan<double, 2> plan({2, 3});
  ABSL_RETURN_IF_ERROR(security::fft::Dct2(absl::MakeConstSpan(block),
                                      absl::MakeSpan(coeffs), plan));
  ABSL_RETURN_IF_ERROR(security::fft::Dct3(absl::MakeConstSpan(coeffs),
                                      absl::MakeSpan(recovered), plan));

  for (double& v : recovered) {
    v /= (2.0 * 2.0) * (2.0 * 3.0);
  }

  std::cout << "\n";
  PrintReal("2D DCT input:     ", block);
  PrintReal("2D DCT-II:        ", coeffs);
  PrintReal("2D DCT-III:       ", recovered);
  return absl::OkStatus();
}

absl::Status RunStridedFft() {
  std::vector<std::complex<double>> interleaved = {
      {1.0, 0.0}, {99.0, 99.0}, {2.0, 0.0}, {99.0, 99.0},
      {3.0, 0.0}, {99.0, 99.0}, {4.0, 0.0}, {99.0, 99.0}};
  std::vector<std::complex<double>> ch0_fft(4);
  std::vector<size_t> nembed = {4};
  security::fft::FftPlan<double, 1> plan({4});

  ABSL_RETURN_IF_ERROR(security::fft::Fft(
      security::fft::ArrayLayout(absl::MakeConstSpan(interleaved), nembed,
                                 /*stride=*/2),
      security::fft::ArrayLayout(absl::MakeSpan(ch0_fft), nembed,
                                 /*stride=*/1),
      plan));

  std::cout << "\n";
  PrintComplex("Strided FFT:      ", ch0_fft);
  return absl::OkStatus();
}

absl::Status Run() {
  ABSL_RETURN_IF_ERROR(RunComplexFftRoundTrip());
  ABSL_RETURN_IF_ERROR(RunLowPassFilterR2c());
  ABSL_RETURN_IF_ERROR(Run2dDctRoundTrip());
  ABSL_RETURN_IF_ERROR(RunStridedFft());
  return absl::OkStatus();
}

}  // namespace

int main() {
  if (absl::Status status = Run(); !status.ok()) {
    std::cerr << status << "\n";
    return 1;
  }
  return 0;
}


