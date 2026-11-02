/// Level matching, kept pure so it can be checked without a decoder.
library;

import 'dart:math' as math;

/// EBU R128 target. -14 LUFS is what streaming services normalise to: quiet
/// enough to leave headroom on a modern master, loud enough that a 1994 CD does
/// not sound muffled next to it.
const double kTargetLufs = -14.0;

/// Bounds of the correction. Pushing a very quiet master by +16 dB exposes its
/// noise floor; pulling a loud one by -20 dB makes it feel broken. Six either
/// way is the range where "same loudness" is audible without being a remaster.
const double kMaxGainDb = 6.0;
const double kMinGainDb = -12.0;

/// How much to add to a track's level, in dB.
double gainForLoudness(double? lufs, {double target = kTargetLufs}) {
  if (lufs == null) return 0;
  return (target - lufs).clamp(kMinGainDb, kMaxGainDb);
}

/// The same correction as a linear multiplier for the audio backend.
double gainFactor(double db) => math.pow(10, db / 20).toDouble();
