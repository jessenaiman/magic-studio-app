// Read native media state without seeking, playing, or changing the source.
export function mediaState(element: Element) {
  if (!/^(VIDEO|AUDIO)$/.test(element.tagName)) return undefined;
  const media = element as HTMLMediaElement;
  const ranges = (value: TimeRanges) => Array.from({ length: value.length }, (_, index) => [value.start(index), value.end(index)]);
  return {
    source: media.currentSrc || media.getAttribute("src") || null,
    currentTime: media.currentTime,
    duration: Number.isFinite(media.duration) ? media.duration : null,
    paused: media.paused,
    ended: media.ended,
    seeking: media.seeking,
    readyState: media.readyState,
    networkState: media.networkState,
    playbackRate: media.playbackRate,
    muted: media.muted,
    volume: media.volume,
    buffered: ranges(media.buffered),
    seekable: ranges(media.seekable),
    videoWidth: element.tagName === "VIDEO" ? (media as HTMLVideoElement).videoWidth : undefined,
    videoHeight: element.tagName === "VIDEO" ? (media as HTMLVideoElement).videoHeight : undefined,
    error: media.error ? { code: media.error.code, message: media.error.message } : null,
  };
}
