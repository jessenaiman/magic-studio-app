import React, { useRef } from "react";
import { LyricDesignItem, WordsWordItem } from "../api";

interface Props { items: LyricDesignItem[]; words: WordsWordItem[]; selected: number | null; time: number; playing: boolean; onSelect: (index: number) => void; onChange: (index: number, patch: Partial<LyricDesignItem>) => void }

export const LyricCanvas: React.FC<Props> = ({ items, words, selected, time, playing, onSelect, onChange }) => {
  const drag = useRef<{ index: number; startX: number; startY: number; x: number; y: number } | null>(null);
  const svg = useRef<SVGSVGElement>(null);
  return <svg ref={svg} id="words-authored-canvas" viewBox="0 0 1920 1080" role="img" aria-label="Authored lyric layout; drag a word to reposition it"
    style={{ width: "100%", height: "100%", touchAction: "none" }}
    onPointerMove={event => { const active = drag.current; if (!active || !svg.current) return;
      const rect = svg.current.getBoundingClientRect();
      onChange(active.index, { x: Math.round(active.x + (event.clientX - active.startX) * 1920 / rect.width), y: Math.round(active.y + (event.clientY - active.startY) * 1080 / rect.height) });
    }} onPointerUp={() => { drag.current = null; }} onPointerCancel={() => { drag.current = null; }}>
    {items.map((item, index) => {
      const word = words.find(word => word.word_id === (item.accent || item.word_ids?.[0]));
      const elapsed = time - (word?.start || 0);
      const keys = item.motion_keys || [];
      let dx = 0, dy = 0, sx = 1, sy = 1, rotation = item.rotate || 0, opacity = 1;
      if (playing && keys.length) {
        const next = keys.findIndex(key => key[0] > elapsed);
        const before = keys[next < 0 ? keys.length - 1 : Math.max(0, next - 1)], after = keys[next < 0 ? keys.length - 1 : next];
        const fraction = before === after ? 0 : Math.max(0, Math.min(1, (elapsed - before[0]) / (after[0] - before[0])));
        const at = (column: number, fallback: number) => (before[column] ?? fallback) + ((after[column] ?? fallback) - (before[column] ?? fallback)) * fraction;
        dx = at(1, 0); dy = at(2, 0); sx = at(3, 100) / 100; sy = at(4, 100) / 100; rotation += at(5, 0); opacity = elapsed < 0 ? 0 : at(6, 100) / 100;
      }
      const font = item.font === "sans" ? "Inter, sans-serif" : item.font === "serif" ? "Georgia, serif" : item.font || "Inter, sans-serif";
      return <g key={index} id={`words-token-${index}`} transform={`translate(${item.x + dx} ${item.y + dy}) rotate(${rotation}) scale(${sx} ${sy})`} opacity={opacity}
        style={{ cursor: playing ? "default" : "move" }} onPointerDown={event => {
          if (playing || !svg.current) return; event.preventDefault(); event.stopPropagation(); onSelect(index);
          drag.current = { index, startX: event.clientX, startY: event.clientY, x: item.x, y: item.y }; svg.current.setPointerCapture(event.pointerId);
        }}>
        <text x={0} y={0} fontSize={item.size || 80} fontFamily={font} fontWeight={item.weight || 700} fontStyle={item.italic ? "italic" : "normal"}
          fill={item.color || "#fff"} stroke={item.outline || "none"} strokeWidth={item.outline ? 2 : 0} paintOrder="stroke" style={{ userSelect: "none" }}>{item.text || item.word_ids?.map(id => words.find(word => word.word_id === id)?.text).join(" ")}</text>
        {selected === index && !playing && <circle cx={-12} cy={-12} r={7} fill="#42d7bd" stroke="#10252d" strokeWidth={2} />}
      </g>;
    })}
  </svg>;
};
