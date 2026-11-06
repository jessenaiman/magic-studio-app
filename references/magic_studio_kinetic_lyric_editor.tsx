/**
 * REFERENCE ONLY: Magic Studio Kinetic Lyric Editor Prototype
 *
 * Working Tauri Implementation:
 * - Desktop Launcher: `magic --lyrics --project <path>` (apps/magic/src/lyrics.rs)
 * - Webview UI: apps/magic-cli/ui/lyrics.html
 * - Word Flow & Kinetic Editor: apps/magic-cli/ui/words-flow.js
 * - Choreography & Stage Design: apps/magic-cli/ui/words-design.js
 * - Engine Bridge & Tools: crates/engine/src/cmd/words/stage.rs (`words.read`, `words.effects`)
 *
 * Tool and effect lists are dynamically populated at runtime from EffectCraft
 * (`effectcraft_host::session()`) and FilmCraft (`filmcraft_engine::Session::default()`).
 */

import React, { useState, useEffect, useRef, useCallback } from 'react';
import {
  Play,
  Pause,
  RotateCcw,
  Sparkles,
  Flame,
  Wind,
  Zap,
  Compass,
  Layers,
  Sliders,
  ChevronDown,
  Search,
  Filter,
  Maximize2,
  Minimize2,
  X,
  Minus,
  Square,
  Anchor,
  Move,
  Spline,
  Repeat,
  Save,
  Share2,
  Eye,
  Volume2,
  Check,
  Code,
  Wand2,
  PenTool,
  Plus,
  Trash2,
  Activity
} from 'lucide-react';

// --- INITIAL SONGS & KINETIC LYRIC DATA ---
const LYRIC_PHRASES = [
  {
    id: 'phrase-1',
    time: '0:08.4',
    startSec: 8.4,
    endSec: 11.9,
    scene: 'Ember Awakening',
    style: 'HeatHaze',
    layout: 'Scattered Ascent',
    bgGradient: 'from-amber-950 via-red-950 to-slate-950',
    sceneFx: {
      atmosphere: 'Ember Vortex',
      beatReactive: 'Shockwave Ring',
      intensity: 0.8
    },
    badges: [
      { label: 'Ignite', icon: 'flame', color: 'text-orange-400 bg-orange-950/70 border-orange-700/50' },
      { label: '2 Drawings', icon: 'sparkles', color: 'text-fuchsia-300 bg-fuchsia-950/70 border-fuchsia-700/50' }
    ],
    words: [
      { id: 'w1-1', text: 'From', effect: 'Fade', choreography: 'Cascade', style: 'Glow', x: 18, y: 26, scale: 1.0, rotate: -6, color: '#cbd5e1' },
      { id: 'w1-2', text: 'the', effect: 'Drift', choreography: 'Cascade', style: 'Glow', x: 34, y: 23, scale: 0.9, rotate: 4, color: '#94a3b8' },
      { id: 'w1-3', text: 'ASHES', effect: 'Impact', choreography: 'Zoom Blast', style: 'Fire', x: 52, y: 48, scale: 1.85, rotate: -4, color: '#fb923c' },
      { id: 'w1-4', text: 'we', effect: 'Float', choreography: 'Spiral', style: 'Sparkle', x: 36, y: 74, scale: 1.05, rotate: 3, color: '#fde047' },
      { id: 'w1-5', text: 'awaken', effect: 'Burst', choreography: 'Spiral', style: 'Glow', x: 64, y: 72, scale: 1.35, rotate: -2, color: '#38bdf8' }
    ],
    drawings: [
      {
        id: 'd1-1',
        name: 'Rising Ash Spiral',
        behavior: 'Trim Draw-On',
        brushStyle: 'Ember Chalk',
        color: '#fb923c',
        strokeWidth: 4,
        startAt: 0.05,
        endAt: 0.85,
        d: 'M 140 360 C 260 390, 290 240, 410 220 C 530 200, 560 95, 670 85'
      },
      {
        id: 'd1-2',
        name: 'Burst Halo Cue',
        behavior: 'Pulse & Boil',
        brushStyle: 'Neon Laser',
        color: '#38bdf8',
        strokeWidth: 3,
        startAt: 0.3,
        endAt: 0.95,
        d: 'M 330 165 C 390 120, 470 120, 520 165 M 310 265 C 390 310, 480 310, 540 260'
      }
    ]
  },
  {
    id: 'phrase-2',
    time: '0:12.4',
    startSec: 12.4,
    endSec: 16.2,
    scene: 'Phoenix Fire',
    style: 'Ignite Kinetic',
    layout: 'Dynamic Shift',
    bgGradient: 'from-orange-950 via-amber-900/60 to-slate-950',
    sceneFx: {
      atmosphere: 'Solar Cinders',
      beatReactive: 'Wing Flare',
      intensity: 0.92
    },
    badges: [
      { label: 'Blast', icon: 'flame', color: 'text-amber-400 bg-amber-950/80 border-amber-600/50' },
      { label: '3 Drawings', icon: 'sparkles', color: 'text-cyan-300 bg-cyan-950/80 border-cyan-600/50' }
    ],
    words: [
      { id: 'w2-1', text: 'I', effect: 'Pop', choreography: 'Cascade', style: 'Glow', x: 20, y: 24, scale: 1.35, rotate: -8, color: '#f8fafc' },
      { id: 'w2-2', text: 'rise', effect: 'Burst', choreography: 'Cascade', style: 'Kinetic Tilt', x: 37, y: 26, scale: 1.45, rotate: -3, color: '#fed7aa' },
      { id: 'w2-3', text: 'like', effect: 'Zoom', choreography: 'Spiral', style: 'Bounce', x: 47, y: 46, scale: 1.15, rotate: 2, color: '#e2e8f0' },
      { id: 'w2-4', text: 'a', effect: 'Drift', choreography: 'Spiral', style: 'Motion Blur', x: 67, y: 45, scale: 1.1, rotate: 6, color: '#94a3b8' },
      { id: 'w2-5', text: 'PHOENIX', effect: 'Impact', choreography: 'Zoom Blast', style: 'Fire', x: 52, y: 72, scale: 2.35, rotate: -7, color: '#f97316' }
    ],
    drawings: [
      {
        id: 'd2-1',
        name: 'Zig-Zag Cue (~~~ >)',
        behavior: 'Trim Draw-On',
        brushStyle: 'Neon Laser',
        color: '#22d3ee',
        strokeWidth: 4,
        startAt: 0.0,
        endAt: 0.75,
        d: 'M 465 135 L 485 110 L 505 140 L 525 110 L 545 140 L 565 115 M 590 112 L 612 126 L 590 140'
      },
      {
        id: 'd2-2',
        name: 'Phoenix Wing Sweep',
        behavior: 'Trim Draw-On',
        brushStyle: 'Fire Ribbon',
        color: '#fb923c',
        strokeWidth: 5,
        startAt: 0.18,
        endAt: 0.95,
        d: 'M 145 355 C 185 225, 295 160, 415 235 C 530 155, 635 215, 685 345'
      },
      {
        id: 'd2-3',
        name: 'Impact Crown Rays',
        behavior: 'Pulse & Boil',
        brushStyle: 'Starlight Ink',
        color: '#fde047',
        strokeWidth: 3.5,
        startAt: 0.35,
        endAt: 1.0,
        d: 'M 245 395 Q 425 425 605 370 M 355 245 L 340 210 M 425 230 L 425 190 M 495 245 L 515 210'
      }
    ]
  },
  {
    id: 'phrase-3',
    time: '0:16.8',
    startSec: 16.8,
    endSec: 20.5,
    scene: 'Starlight Ridge',
    style: 'Cosmic Fantasy',
    layout: 'Arc Orbit',
    bgGradient: 'from-indigo-950 via-purple-950 to-slate-950',
    sceneFx: {
      atmosphere: 'Aurora Dust',
      beatReactive: 'Starlight Ripple',
      intensity: 0.75
    },
    badges: [
      { label: 'Rising', icon: 'sparkles', color: 'text-emerald-400 bg-emerald-950/70 border-emerald-700/50' },
      { label: '2 Drawings', icon: 'wind', color: 'text-teal-300 bg-teal-950/70 border-teal-700/50' }
    ],
    words: [
      { id: 'w3-1', text: 'High', effect: 'Float', choreography: 'Cascade', style: 'Glow', x: 22, y: 28, scale: 1.4, rotate: -10, color: '#38bdf8' },
      { id: 'w3-2', text: 'above', effect: 'Drift', choreography: 'Cascade', style: 'Sparkle', x: 45, y: 22, scale: 1.25, rotate: -2, color: '#818cf8' },
      { id: 'w3-3', text: 'the', effect: 'Fade', choreography: 'Spiral', style: 'Glow', x: 66, y: 29, scale: 0.95, rotate: 5, color: '#cbd5e1' },
      { id: 'w3-4', text: 'MOUNTAINS', effect: 'Zoom', choreography: 'Zoom Blast', style: 'Kinetic Tilt', x: 50, y: 64, scale: 1.95, rotate: -3, color: '#c084fc' }
    ],
    drawings: [
      {
        id: 'd3-1',
        name: 'Constellation Orbit',
        behavior: 'Trim Draw-On',
        brushStyle: 'Neon Laser',
        color: '#38bdf8',
        strokeWidth: 3.5,
        startAt: 0.0,
        endAt: 0.9,
        d: 'M 120 260 Q 400 45 680 250'
      },
      {
        id: 'd3-2',
        name: 'Ridge Echo Lines',
        behavior: 'Wave Flow',
        brushStyle: 'Starlight Ink',
        color: '#c084fc',
        strokeWidth: 3,
        startAt: 0.25,
        endAt: 1.0,
        d: 'M 180 355 L 290 295 L 405 345 L 535 280 L 645 350'
      }
    ]
  },
  {
    id: 'phrase-4',
    time: '0:21.0',
    startSec: 21.0,
    endSec: 25.0,
    scene: 'Solar Flare',
    style: 'Supernova',
    layout: 'Radial Burst',
    bgGradient: 'from-rose-950 via-amber-950 to-slate-950',
    sceneFx: {
      atmosphere: 'Solar Cinders',
      beatReactive: 'Shockwave Ring',
      intensity: 0.95
    },
    badges: [
      { label: 'Fires', icon: 'flame', color: 'text-rose-400 bg-rose-950/70 border-rose-700/50' },
      { label: '1 Drawing', icon: 'zap', color: 'text-amber-300 bg-amber-950/70 border-amber-700/50' }
    ],
    words: [
      { id: 'w4-1', text: 'Burning', effect: 'Impact', choreography: 'Zoom Blast', style: 'Fire', x: 32, y: 32, scale: 1.65, rotate: -6, color: '#fb7185' },
      { id: 'w4-2', text: 'through', effect: 'Zoom', choreography: 'Spiral', style: 'Shake', x: 64, y: 36, scale: 1.15, rotate: 4, color: '#fda4af' },
      { id: 'w4-3', text: 'the', effect: 'Pop', choreography: 'Cascade', style: 'Glow', x: 28, y: 64, scale: 1.0, rotate: -3, color: '#cbd5e1' },
      { id: 'w4-4', text: 'STARLIGHT', effect: 'Burst', choreography: 'Zoom Blast', style: 'Sparkle', x: 60, y: 68, scale: 1.9, rotate: -5, color: '#fde047' }
    ],
    drawings: [
      {
        id: 'd4-1',
        name: 'Supernova Vortex',
        behavior: 'Trim Draw-On',
        brushStyle: 'Fire Ribbon',
        color: '#fb7185',
        strokeWidth: 4.5,
        startAt: 0.1,
        endAt: 0.95,
        d: 'M 150 110 C 380 40, 440 390, 670 320'
      }
    ]
  },
  {
    id: 'phrase-5',
    time: '0:25.4',
    startSec: 25.4,
    endSec: 29.8,
    scene: 'Nebula Horizon',
    style: 'Zero Gravity',
    layout: 'Wave Cascade',
    bgGradient: 'from-cyan-950 via-blue-950 to-slate-950',
    sceneFx: {
      atmosphere: 'Aurora Dust',
      beatReactive: 'Horizon Pulse',
      intensity: 0.7
    },
    badges: [
      { label: 'Trail', icon: 'wind', color: 'text-cyan-400 bg-cyan-950/70 border-cyan-700/50' },
      { label: '1 Drawing', icon: 'sparkles', color: 'text-indigo-400 bg-indigo-950/70 border-indigo-700/50' }
    ],
    words: [
      { id: 'w5-1', text: 'Into', effect: 'Drift', choreography: 'Cascade', style: 'Glow', x: 25, y: 34, scale: 1.2, rotate: -4, color: '#67e8f9' },
      { id: 'w5-2', text: 'the', effect: 'Float', choreography: 'Cascade', style: 'Glow', x: 46, y: 32, scale: 1.0, rotate: 2, color: '#94a3b8' },
      { id: 'w5-3', text: 'ENDLESS', effect: 'Burst', choreography: 'Spiral', style: 'Kinetic Tilt', x: 44, y: 62, scale: 1.85, rotate: -5, color: '#38bdf8' },
      { id: 'w5-4', text: 'sky', effect: 'Float', choreography: 'Zoom Blast', style: 'Sparkle', x: 77, y: 64, scale: 1.45, rotate: 6, color: '#a5f3fc' }
    ],
    drawings: [
      {
        id: 'd5-1',
        name: 'Wind Slipstream',
        behavior: 'Wave Flow',
        brushStyle: 'Neon Laser',
        color: '#67e8f9',
        strokeWidth: 3.5,
        startAt: 0.0,
        endAt: 1.0,
        d: 'M 95 220 C 260 120, 520 330, 710 190'
      }
    ]
  }
];

const DRAWING_BEHAVIORS = [
  { id: 'Trim Draw-On', label: 'Trim Draw-On', desc: 'Animates stroke path along playhead' },
  { id: 'Pulse & Boil', label: 'Pulse & Boil', desc: 'Hand-drawn frame jitter + beat glow' },
  { id: 'Wave Flow', label: 'Wave Flow', desc: 'Continuous kinetic ribbon undulation' }
];

const DRAWING_BRUSH_STYLES = [
  { id: 'Neon Laser', label: 'Neon Laser', badge: '⚡', color: '#22d3ee' },
  { id: 'Fire Ribbon', label: 'Fire Ribbon', badge: '🔥', color: '#fb923c' },
  { id: 'Starlight Ink', label: 'Starlight Ink', badge: '✨', color: '#fde047' },
  { id: 'Ember Chalk', label: 'Ember Chalk', badge: '🖍️', color: '#fb7185' }
];

const MOTION_DOODLE_STAMPS = [
  {
    name: 'Zig-Zag Cue (~~~ >)',
    color: '#22d3ee',
    brushStyle: 'Neon Laser',
    behavior: 'Trim Draw-On',
    d: 'M 455 125 L 475 100 L 495 130 L 515 100 L 535 130 L 555 105 M 580 102 L 602 116 L 580 130'
  },
  {
    name: 'Orbit Swoosh',
    color: '#fb923c',
    brushStyle: 'Fire Ribbon',
    behavior: 'Trim Draw-On',
    d: 'M 140 320 C 260 90, 540 90, 660 320'
  },
  {
    name: 'Crown Rays',
    color: '#fde047',
    brushStyle: 'Starlight Ink',
    behavior: 'Pulse & Boil',
    d: 'M 330 185 L 305 140 M 400 165 L 400 115 M 470 185 L 495 140'
  },
  {
    name: 'Shockwave Under',
    color: '#c084fc',
    brushStyle: 'Neon Laser',
    behavior: 'Wave Flow',
    d: 'M 210 385 Q 400 430 590 385 M 260 405 Q 400 440 540 405'
  }
];

const SCENE_ATMOSPHERES = ['Solar Cinders', 'Ember Vortex', 'Aurora Dust', 'Cyber Rain'];
const BEAT_REACTIVE_FX = ['Wing Flare', 'Shockwave Ring', 'Starlight Ripple', 'Horizon Pulse'];

const CHOREOGRAPHY_OPTIONS = [
  { id: 'Cascade', label: 'Cascade', desc: 'Staggered sequential entry' },
  { id: 'Spiral', label: 'Spiral', desc: 'Vortex orbital trajectory' },
  { id: 'Zoom Blast', label: 'Zoom Blast', desc: 'High-impact Z-axis punch' }
];

const MOTION_STYLES = [
  { id: 'Bounce', label: 'Bounce', badge: '⚡' },
  { id: 'Shake', label: 'Shake', badge: '〰️' },
  { id: 'Kinetic Tilt', label: 'Kinetic Tilt', badge: '📐' },
  { id: 'Fire', label: 'Fire Shader', badge: '🔥' },
  { id: 'Glow', label: 'Neon Glow', badge: '✨' },
  { id: 'Sparkle', label: 'Sparkle', badge: '🌟' }
];

const GLOBAL_PRESETS = [
  { id: 'Ignite Kinetic', name: 'Ignite Kinetic', desc: 'Fiery bevels, intense ember trails & Z-punch' },
  { id: 'Cosmic Fantasy', name: 'Cosmic Fantasy', desc: 'Soft starlight bloom with zero-gravity drift' },
  { id: 'Cyber Glint', name: 'Cyber Glint', desc: 'Chromatic aberration with razor-sharp cuts' },
  { id: 'Storybook', name: 'Storybook Ink', desc: 'Warm paper grain with organic letter bounce' }
];

export default function App() {
  // --- STATE ---
  const [phrases, setPhrases] = useState(LYRIC_PHRASES);
  const [activePhraseId, setActivePhraseId] = useState('phrase-2');
  const [selectedWordId, setSelectedWordId] = useState('w2-5'); // Default to "PHOENIX"
  const [selectedDrawingId, setSelectedDrawingId] = useState('d2-2'); // Default to "Phoenix Wing Sweep"
  const [inspectorMode, setInspectorMode] = useState('motion-drawings'); // 'motion-drawings' | 'word'
  const [isDrawingMode, setIsDrawingMode] = useState(false);
  const [currentStrokePoints, setCurrentStrokePoints] = useState([]);
  const [brushColor, setBrushColor] = useState('#22d3ee');
  const [isPlaying, setIsPlaying] = useState(true);
  const [playhead, setPlayhead] = useState(0.45); // 0 to 1 progress inside active phrase
  const [searchQuery, setSearchQuery] = useState('');
  const [showPresetDropdown, setShowPresetDropdown] = useState(false);
  const [showWarpPaths, setShowWarpPaths] = useState(true);
  const [showTethers, setShowTethers] = useState(true);
  const [showTauriModal, setShowTauriModal] = useState(false);
  const [activeTopTab, setActiveTopTab] = useState('Motion');

  // Bezier Curve Editor Handles (0 to 1 normalized space)
  const [cp1, setCp1] = useState({ x: 0.28, y: 0.85 });
  const [cp2, setCp2] = useState({ x: 0.68, y: 0.12 });
  const [draggingHandle, setDraggingHandle] = useState(null);

  // Dragging word on canvas
  const [draggingWordId, setDraggingWordId] = useState(null);

  // Refs for calculating dynamic cross-panel SVG tether coordinates
  const workspaceRef = useRef(null);
  const cardRefs = useRef({});
  const wordRefs = useRef({});
  const stageRef = useRef(null);
  const scrubberDotRef = useRef(null);
  const rightTrackNodeRef = useRef(null);
  const svgCurveRef = useRef(null);

  const [tetherCoords, setTetherCoords] = useState(null);

  const activePhrase = phrases.find((p) => p.id === activePhraseId) || phrases[1];
  const selectedWord =
    activePhrase.words.find((w) => w.id === selectedWordId) || activePhrase.words[0];
  const selectedDrawing =
    (activePhrase.drawings || []).find((d) => d.id === selectedDrawingId) ||
    activePhrase.drawings?.[0];

  // Ensure selectedWordId & selectedDrawingId stay valid when switching phrases
  useEffect(() => {
    const wordExists = activePhrase.words.some((w) => w.id === selectedWordId);
    if (!wordExists && activePhrase.words.length > 0) {
      setSelectedWordId(activePhrase.words[activePhrase.words.length - 1].id);
    }
    const drawingExists = (activePhrase.drawings || []).some((d) => d.id === selectedDrawingId);
    if (!drawingExists && activePhrase.drawings?.length > 0) {
      setSelectedDrawingId(activePhrase.drawings[0].id);
    }
  }, [activePhraseId, activePhrase, selectedWordId, selectedDrawingId]);

  // Playback loop for kinetic preview
  useEffect(() => {
    if (!isPlaying) return;
    const interval = setInterval(() => {
      setPlayhead((prev) => {
        const next = prev + 0.015;
        return next > 1 ? 0 : +next.toFixed(3);
      });
    }, 50);
    return () => clearInterval(interval);
  }, [isPlaying]);

  // Calculate dynamic cross-panel SVG tether coordinates
  const updateTetherCoordinates = useCallback(() => {
    if (!workspaceRef.current) return;
    const wsRect = workspaceRef.current.getBoundingClientRect();

    const activeCardEl = cardRefs.current[activePhraseId];
    const firstWordId = activePhrase.words[0]?.id;
    const firstWordEl = firstWordId ? wordRefs.current[firstWordId] : null;
    const selectedWordEl = selectedWord?.id ? wordRefs.current[selectedWord.id] : null;
    const scrubberEl = scrubberDotRef.current;
    const rightNodeEl = rightTrackNodeRef.current;

    if (!activeCardEl || !firstWordEl || !selectedWordEl || !scrubberEl || !rightNodeEl) {
      return;
    }

    const cardRect = activeCardEl.getBoundingClientRect();
    const firstWordRect = firstWordEl.getBoundingClientRect();
    const selectedWordRect = selectedWordEl.getBoundingClientRect();
    const scrubberRect = scrubberEl.getBoundingClientRect();
    const rightNodeRect = rightNodeEl.getBoundingClientRect();

    setTetherCoords({
      leftAnchor: {
        x: cardRect.right - wsRect.left - 8,
        y: cardRect.top - wsRect.top + cardRect.height / 2
      },
      wordTarget1: {
        x: firstWordRect.left - wsRect.left,
        y: firstWordRect.top - wsRect.top + firstWordRect.height / 2
      },
      wordTarget2: {
        x: selectedWordRect.left - wsRect.left,
        y: selectedWordRect.top - wsRect.top + selectedWordRect.height / 2
      },
      wordRightAnchor: {
        x: selectedWordRect.right - wsRect.left,
        y: selectedWordRect.top - wsRect.top + selectedWordRect.height / 2
      },
      scrubberAnchor: {
        x: scrubberRect.left - wsRect.left + scrubberRect.width / 2,
        y: scrubberRect.top - wsRect.top + scrubberRect.height / 2
      },
      rightTimelineAnchor: {
        x: rightNodeRect.left - wsRect.left + rightNodeRect.width / 2,
        y: rightNodeRect.top - wsRect.top + rightNodeRect.height / 2
      }
    });
  }, [activePhraseId, activePhrase, selectedWord]);

  useEffect(() => {
    const timer = setTimeout(updateTetherCoordinates, 40);
    window.addEventListener('resize', updateTetherCoordinates);
    return () => {
      clearTimeout(timer);
      window.removeEventListener('resize', updateTetherCoordinates);
    };
  }, [updateTetherCoordinates, phrases, inspectorMode]);

  // Handle dragging words or sketching motion drawings on the canvas stage
  const handleStagePointerDown = (e) => {
    if (!isDrawingMode || !stageRef.current) return;
    const rect = stageRef.current.getBoundingClientRect();
    const x = Math.round(((e.clientX - rect.left) / rect.width) * 800);
    const y = Math.round(((e.clientY - rect.top) / rect.height) * 450);
    setCurrentStrokePoints([{ x, y }]);
  };

  const handleStagePointerMove = (e) => {
    if (isDrawingMode && currentStrokePoints.length > 0 && stageRef.current) {
      const rect = stageRef.current.getBoundingClientRect();
      const x = Math.round(((e.clientX - rect.left) / rect.width) * 800);
      const y = Math.round(((e.clientY - rect.top) / rect.height) * 450);
      setCurrentStrokePoints((prev) => [...prev, { x, y }]);
      return;
    }

    if (draggingWordId && stageRef.current) {
      const rect = stageRef.current.getBoundingClientRect();
      const xPct = Math.max(12, Math.min(88, ((e.clientX - rect.left) / rect.width) * 100));
      const yPct = Math.max(16, Math.min(84, ((e.clientY - rect.top) / rect.height) * 100));

      setPhrases((prev) =>
        prev.map((p) => {
          if (p.id !== activePhraseId) return p;
          return {
            ...p,
            words: p.words.map((w) =>
              w.id === draggingWordId ? { ...w, x: Math.round(xPct), y: Math.round(yPct) } : w
            )
          };
        })
      );
    }

    if (draggingHandle && svgCurveRef.current) {
      const rect = svgCurveRef.current.getBoundingClientRect();
      const nx = Math.max(0.05, Math.min(0.95, (e.clientX - rect.left) / rect.width));
      const ny = Math.max(0.05, Math.min(0.95, 1 - (e.clientY - rect.top) / rect.height));
      if (draggingHandle === 'cp1') setCp1({ x: +nx.toFixed(2), y: +ny.toFixed(2) });
      if (draggingHandle === 'cp2') setCp2({ x: +nx.toFixed(2), y: +ny.toFixed(2) });
    }
  };

  const handleGlobalPointerUp = () => {
    if (isDrawingMode && currentStrokePoints.length > 2) {
      const pathString = currentStrokePoints
        .map((pt, i) => `${i === 0 ? 'M' : 'L'} ${pt.x} ${pt.y}`)
        .join(' ');
      const newId = `d-custom-${Date.now()}`;
      const newDrawing = {
        id: newId,
        name: `Motion Sketch #${(activePhrase.drawings?.length || 0) + 1}`,
        behavior: 'Trim Draw-On',
        brushStyle: 'Neon Laser',
        color: brushColor,
        strokeWidth: 4,
        startAt: 0.0,
        endAt: 0.95,
        d: pathString
      };
      setPhrases((prev) =>
        prev.map((p) =>
          p.id === activePhraseId
            ? { ...p, drawings: [...(p.drawings || []), newDrawing] }
            : p
        )
      );
      setSelectedDrawingId(newId);
      setInspectorMode('motion-drawings');
    }
    if (currentStrokePoints.length > 0) setCurrentStrokePoints([]);
    if (draggingWordId) setDraggingWordId(null);
    if (draggingHandle) setDraggingHandle(null);
  };

  // Update properties on the currently selected word
  const updateSelectedWordProp = (key, value) => {
    setPhrases((prev) =>
      prev.map((p) => {
        if (p.id !== activePhraseId) return p;
        return {
          ...p,
          words: p.words.map((w) => (w.id === selectedWord.id ? { ...w, [key]: value } : w))
        };
      })
    );
  };

  // Update properties on the currently selected complementary drawing
  const updateSelectedDrawingProp = (key, value) => {
    if (!selectedDrawing) return;
    setPhrases((prev) =>
      prev.map((p) => {
        if (p.id !== activePhraseId) return p;
        return {
          ...p,
          drawings: (p.drawings || []).map((d) =>
            d.id === selectedDrawing.id ? { ...d, [key]: value } : d
          )
        };
      })
    );
  };

  // Update complementary scene FX (Atmosphere / Beat Reactive)
  const updateSceneFxProp = (key, value) => {
    setPhrases((prev) =>
      prev.map((p) =>
        p.id === activePhraseId
          ? { ...p, sceneFx: { ...(p.sceneFx || {}), [key]: value } }
          : p
      )
    );
  };

  // Add a preset complementary motion doodle stamp
  const addDoodleStamp = (stamp) => {
    const newId = `d-stamp-${Date.now()}`;
    const newDrawing = {
      id: newId,
      name: stamp.name,
      behavior: stamp.behavior,
      brushStyle: stamp.brushStyle,
      color: stamp.color,
      strokeWidth: 4,
      startAt: 0.05,
      endAt: 0.92,
      d: stamp.d
    };
    setPhrases((prev) =>
      prev.map((p) =>
        p.id === activePhraseId
          ? { ...p, drawings: [...(p.drawings || []), newDrawing] }
          : p
      )
    );
    setSelectedDrawingId(newId);
    setInspectorMode('motion-drawings');
  };

  // Delete selected drawing
  const deleteSelectedDrawing = () => {
    if (!selectedDrawing) return;
    setPhrases((prev) =>
      prev.map((p) => {
        if (p.id !== activePhraseId) return p;
        const remaining = (p.drawings || []).filter((d) => d.id !== selectedDrawing.id);
        return { ...p, drawings: remaining };
      })
    );
  };

  // Update phrase preset
  const applyPresetToPhrase = (presetName) => {
    setPhrases((prev) =>
      prev.map((p) => (p.id === activePhraseId ? { ...p, style: presetName } : p))
    );
    setShowPresetDropdown(false);
  };

  const filteredPhrases = phrases.filter((p) => {
    const joined = p.words.map((w) => w.text).join(' ').toLowerCase();
    return (
      joined.includes(searchQuery.toLowerCase()) ||
      p.scene.toLowerCase().includes(searchQuery.toLowerCase())
    );
  });

  // Helper to build smooth cubic bezier SVG path string
  const buildBezier = (start, end, curvature = 0.45) => {
    const dx = Math.abs(end.x - start.x) * curvature;
    return `M ${start.x} ${start.y} C ${start.x + dx} ${start.y}, ${end.x - dx} ${end.y}, ${end.x} ${end.y}`;
  };

  // Current time formatted
  const currentTimestamp = (
    activePhrase.startSec +
    playhead * (activePhrase.endSec - activePhrase.startSec)
  ).toFixed(1);

  return (
    <div
      className="h-screen w-screen overflow-hidden bg-[#090d14] text-slate-100 flex flex-col select-none font-sans antialiased"
      onPointerMove={handleStagePointerMove}
      onPointerUp={handleGlobalPointerUp}
    >
      {/* =====================================================================
          TAURI FRAMELESS DESKTOP TITLEBAR (data-tauri-drag-region)
      ====================================================================== */}
      <header
        data-tauri-drag-region
        className="h-11 bg-[#06090e] border-b border-slate-800/80 px-3 flex items-center justify-between shrink-0 z-40"
      >
        {/* Left: App Brand & File Menu */}
        <div className="flex items-center space-x-4" data-tauri-drag-region>
          <div className="flex items-center space-x-2">
            <div className="w-6 h-6 rounded-md bg-gradient-to-tr from-cyan-500 via-teal-400 to-amber-400 flex items-center justify-center shadow-md shadow-cyan-500/20">
              <Wand2 className="w-3.5 h-3.5 text-slate-950 stroke-[2.5]" />
            </div>
            <span className="font-bold tracking-tight text-sm bg-gradient-to-r from-white via-slate-100 to-cyan-300 bg-clip-text text-transparent">
              Magic Studio
            </span>
            <span className="text-[10px] font-mono uppercase px-1.5 py-0.5 rounded bg-cyan-950/90 text-cyan-300 border border-cyan-700/40">
              Tauri v2
            </span>
          </div>

          <div className="h-3.5 w-[1px] bg-slate-800" />

          <div className="flex items-center space-x-2 text-xs text-slate-400">
            <button className="hover:text-white px-2 py-1 rounded hover:bg-slate-800/60 transition flex items-center gap-1">
              <span>File</span>
              <ChevronDown className="w-3 h-3" />
            </button>
            <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-slate-900/90 border border-slate-800/80 text-slate-300">
              <Flame className="w-3.5 h-3.5 text-amber-400" />
              <span className="text-xs font-medium">Phoenix_Kinetic_Master.mproj</span>
            </div>
          </div>
        </div>

        {/* Center: Mode Switcher Tabs */}
        <nav className="flex items-center space-x-1 bg-slate-900/90 p-1 rounded-lg border border-slate-800/80">
          {['Storyboard', 'Animation', 'Lyrics', 'Motion'].map((tab) => {
            const active = activeTopTab === tab;
            return (
              <button
                key={tab}
                onClick={() => {
                  setActiveTopTab(tab);
                  if (tab === 'Motion' || tab === 'Animation') {
                    setInspectorMode('motion-drawings');
                  } else if (tab === 'Lyrics') {
                    setInspectorMode('word');
                    setIsDrawingMode(false);
                  }
                }}
                className={`px-3.5 py-1 rounded-md text-xs font-medium transition-all relative ${
                  active
                    ? 'text-cyan-300 bg-cyan-950/60 border border-cyan-500/40 shadow-sm shadow-cyan-500/10'
                    : 'text-slate-400 hover:text-slate-200'
                }`}
              >
                {tab}
                {active && (
                  <span className="absolute -bottom-1 left-2 right-2 h-0.5 bg-cyan-400 rounded-full shadow-[0_0_8px_#22d3ee]" />
                )}
              </button>
            );
          })}
        </nav>

        {/* Right: Tauri Spec Button & Native Window Controls */}
        <div className="flex items-center space-x-2">
          <button
            onClick={() => setShowTethers(!showTethers)}
            className={`px-2.5 py-1 rounded-md text-xs font-medium border transition flex items-center gap-1.5 ${
              showTethers
                ? 'bg-cyan-950/60 border-cyan-600/50 text-cyan-300'
                : 'bg-slate-900 border-slate-800 text-slate-400'
            }`}
            title="Toggle Cross-Panel Motion Tethers"
          >
            <Anchor className="w-3.5 h-3.5" />
            <span>Tethers</span>
          </button>

          <button
            onClick={() => setShowTauriModal(true)}
            className="px-2.5 py-1 rounded-md text-xs font-medium bg-slate-900 hover:bg-slate-800 text-slate-300 border border-slate-700/70 flex items-center gap-1.5 transition"
          >
            <Code className="w-3.5 h-3.5 text-amber-400" />
            <span>Dev & Tauri Spec</span>
          </button>

          <div className="h-3.5 w-[1px] bg-slate-800 mx-1" />

          {/* Native-style Window Buttons */}
          <div className="flex items-center space-x-1">
            <button className="p-1.5 text-slate-400 hover:text-white hover:bg-slate-800 rounded transition">
              <Minus className="w-3.5 h-3.5" />
            </button>
            <button className="p-1.5 text-slate-400 hover:text-white hover:bg-slate-800 rounded transition">
              <Square className="w-3 h-3" />
            </button>
            <button className="p-1.5 text-slate-400 hover:text-rose-400 hover:bg-rose-950/50 rounded transition">
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        </div>
      </header>

      {/* =====================================================================
          MAIN 3-COLUMN WORKSPACE + SVG TETHER OVERLAY
      ====================================================================== */}
      <div ref={workspaceRef} className="flex-1 flex relative overflow-hidden">
        {/* -------------------------------------------------------------------
            DYNAMIC CROSS-PANEL BEZIER TETHER OVERLAY (SVG)
        -------------------------------------------------------------------- */}
        {showTethers && tetherCoords && (
          <svg
            className="pointer-events-none absolute inset-0 w-full h-full z-30 overflow-visible"
            style={{ filter: 'drop-shadow(0 0 6px rgba(34, 211, 238, 0.45))' }}
          >
            <defs>
              <linearGradient id="cyanTetherGrad" x1="0%" y1="0%" x2="100%" y2="0%">
                <stop offset="0%" stopColor="#22d3ee" stopOpacity="0.95" />
                <stop offset="60%" stopColor="#38bdf8" stopOpacity="0.75" />
                <stop offset="100%" stopColor="#f97316" stopOpacity="0.9" />
              </linearGradient>
              <linearGradient id="rightTetherGrad" x1="0%" y1="0%" x2="100%" y2="0%">
                <stop offset="0%" stopColor="#f97316" stopOpacity="0.85" />
                <stop offset="50%" stopColor="#22d3ee" stopOpacity="0.85" />
                <stop offset="100%" stopColor="#22d3ee" stopOpacity="1" />
              </linearGradient>
            </defs>

            {/* Left Active Card -> First Word in Kinetic Phrase */}
            <path
              d={buildBezier(tetherCoords.leftAnchor, tetherCoords.wordTarget1, 0.5)}
              fill="none"
              stroke="#22d3ee"
              strokeWidth="1.75"
              strokeDasharray="4 4"
              className="opacity-75"
            />
            <circle
              cx={tetherCoords.wordTarget1.x}
              cy={tetherCoords.wordTarget1.y}
              r="3.5"
              fill="#22d3ee"
            />

            {/* Left Active Card -> Currently Selected Kinetic Word ("PHOENIX") */}
            <path
              d={buildBezier(tetherCoords.leftAnchor, tetherCoords.wordTarget2, 0.45)}
              fill="none"
              stroke="url(#cyanTetherGrad)"
              strokeWidth="2.5"
            />
            <circle
              cx={tetherCoords.leftAnchor.x}
              cy={tetherCoords.leftAnchor.y}
              r="5"
              fill="#06b6d4"
              stroke="#ecfeff"
              strokeWidth="1.5"
            />
            <circle
              cx={tetherCoords.wordTarget2.x}
              cy={tetherCoords.wordTarget2.y}
              r="4.5"
              fill="#fb923c"
              stroke="#fff7ed"
              strokeWidth="1.5"
            />

            {/* Selected Kinetic Word -> Right Vertical Keyframe Block */}
            <path
              d={buildBezier(tetherCoords.wordRightAnchor, tetherCoords.rightTimelineAnchor, 0.48)}
              fill="none"
              stroke="url(#rightTetherGrad)"
              strokeWidth="2.2"
            />
            <circle
              cx={tetherCoords.rightTimelineAnchor.x}
              cy={tetherCoords.rightTimelineAnchor.y}
              r="5"
              fill="#22d3ee"
              stroke="#ecfeff"
              strokeWidth="1.5"
            />

            {/* Video Scrubber Playhead -> Right Vertical Timeline Sync Line */}
            <path
              d={buildBezier(tetherCoords.scrubberAnchor, tetherCoords.rightTimelineAnchor, 0.35)}
              fill="none"
              stroke="#22d3ee"
              strokeWidth="1.4"
              strokeDasharray="3 3"
              className="opacity-65"
            />
          </svg>
        )}

        {/* ===================================================================
            1. LEFT PANEL: COMPACT SCENE-AWARE KINETIC SCRIPT LIST
        ==================================================================== */}
        <aside className="w-80 bg-[#0b1018] border-r border-slate-800/80 flex flex-col shrink-0 z-20">
          {/* Search & Filter Header */}
          <div className="p-3 border-b border-slate-800/80 space-y-2.5">
            <div className="flex items-center space-x-2">
              <div className="relative flex-1">
                <Search className="w-3.5 h-3.5 text-slate-500 absolute left-2.5 top-1/2 -translate-y-1/2" />
                <input
                  type="text"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder="Search verses, scenes, effects..."
                  className="w-full bg-slate-900/90 border border-slate-800 focus:border-cyan-500/60 rounded-lg pl-8 pr-3 py-1.5 text-xs text-slate-200 placeholder-slate-500 focus:outline-none transition"
                />
              </div>
              <button
                className="p-1.5 rounded-lg bg-slate-900 border border-slate-800 text-slate-400 hover:text-slate-200 transition"
                title="Filter by Motion Tag"
              >
                <Filter className="w-3.5 h-3.5" />
              </button>
            </div>

            <div className="flex items-center justify-between px-0.5">
              <span className="text-xs font-semibold uppercase tracking-wider text-slate-400">
                Verses &amp; Kinetic Phrases
              </span>
              <span className="text-[11px] font-mono text-slate-500">
                {filteredPhrases.length} Phrases
              </span>
            </div>
          </div>

          {/* Scrollable Storyboard / Lyric Phrase Cards */}
          <div
            onScroll={updateTetherCoordinates}
            className="flex-1 overflow-y-auto p-2.5 space-y-2.5 custom-scrollbar"
          >
            {filteredPhrases.map((phrase) => {
              const isSelected = phrase.id === activePhraseId;
              const fullLine = phrase.words.map((w) => w.text).join(' ');

              return (
                <div
                  key={phrase.id}
                  ref={(el) => (cardRefs.current[phrase.id] = el)}
                  onClick={() => {
                    setActivePhraseId(phrase.id);
                    setPlayhead(0.45);
                  }}
                  className={`group relative rounded-xl p-2.5 cursor-pointer transition-all border flex items-center gap-3 ${
                    isSelected
                      ? 'bg-slate-900/95 border-cyan-400/90 shadow-[0_0_20px_rgba(34,211,238,0.18)] ring-1 ring-cyan-400/50'
                      : 'bg-slate-900/40 border-slate-800/80 hover:bg-slate-900/75 hover:border-slate-700'
                  }`}
                >
                  {/* Active Left Accent Bar */}
                  {isSelected && (
                    <div className="absolute left-0 top-2 bottom-2 w-1 bg-cyan-400 rounded-r-full shadow-[0_0_8px_#22d3ee]" />
                  )}

                  {/* Contextual Mini Scene Thumbnail (Replaces noisy checkerboard) */}
                  <div
                    className={`w-16 h-12 rounded-lg bg-gradient-to-br ${phrase.bgGradient} border border-white/10 shrink-0 relative overflow-hidden flex items-center justify-center shadow-inner`}
                  >
                    {/* Stylized mini phoenix/scene silhouette */}
                    <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(251,146,60,0.35),transparent_70%)]" />
                    <span
                      className="text-[10px] font-black tracking-tighter uppercase text-amber-200/90 drop-shadow-[0_1px_4px_rgba(249,115,22,0.8)] transform -rotate-6 select-none truncate px-1"
                    >
                      {phrase.words[phrase.words.length - 1]?.text}
                    </span>
                    <span className="absolute bottom-0.5 right-1 text-[8px] font-mono text-white/70 bg-black/50 px-1 rounded">
                      16:9
                    </span>
                  </div>

                  {/* Lyric Line & Applied Motion Badges */}
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-1.5">
                      <span className="text-[11px] font-mono text-cyan-400/90 font-medium">
                        [{phrase.time}]
                      </span>
                      <p className="text-xs font-semibold text-slate-100 truncate">
                        {fullLine}
                      </p>
                    </div>

                    <div className="flex items-center gap-1.5 mt-1.5 flex-wrap">
                      {phrase.badges.map((b, idx) => (
                        <span
                          key={idx}
                          className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium border ${b.color}`}
                        >
                          {b.icon === 'flame' && <Flame className="w-2.5 h-2.5" />}
                          {b.icon === 'wind' && <Wind className="w-2.5 h-2.5" />}
                          {b.icon === 'sparkles' && <Sparkles className="w-2.5 h-2.5" />}
                          {b.icon === 'zap' && <Zap className="w-2.5 h-2.5" />}
                          {b.label}
                        </span>
                      ))}
                      <span className="text-[10px] text-slate-500 ml-auto truncate">
                        {phrase.scene}
                      </span>
                    </div>
                  </div>

                  {/* Right Tether Anchor Socket on Active Card */}
                  {isSelected && (
                    <div
                      className="w-5 h-5 rounded-full bg-cyan-950 border border-cyan-400 flex items-center justify-center text-cyan-300 shadow-[0_0_10px_#22d3ee] shrink-0"
                      title="Tethered to Center Stage"
                    >
                      <Anchor className="w-2.5 h-2.5" />
                    </div>
                  )}
                </div>
              );
            })}
          </div>

          {/* Left Footer Summary */}
          <div className="p-2.5 border-t border-slate-800/80 bg-[#080c12] flex items-center justify-between text-[11px] text-slate-400">
            <span>Track: Lead Vocals (Stem 1)</span>
            <span className="text-emerald-400 flex items-center gap-1">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
              Beat-Synced
            </span>
          </div>
        </aside>

        {/* ===================================================================
            2. CENTER WORKSPACE: COMPACT CONTEXT BAR + KINETIC STAGE + ENGINE
        ==================================================================== */}
        <main className="flex-1 flex flex-col bg-[#090d14] min-w-0 z-10 px-5 py-3 justify-between overflow-y-auto">
          {/* -----------------------------------------------------------------
              TOP BAR: BREADCRUMB + COLLAPSED SCENE/STYLE PILLS (No bulky white row)
          ------------------------------------------------------------------ */}
          <div className="flex items-center justify-between gap-2 pb-2 border-b border-slate-800/70 shrink-0">
            <div className="flex items-center gap-2 flex-wrap">
              {/* Scene Pill */}
              <div className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs bg-cyan-950/50 border border-cyan-500/40 text-cyan-200">
                <Layers className="w-3.5 h-3.5 text-cyan-400" />
                <span className="text-slate-400">Scene:</span>
                <span className="font-semibold">{activePhrase.scene}</span>
              </div>

              {/* Style Pill */}
              <div className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs bg-amber-950/40 border border-amber-500/40 text-amber-200">
                <Sparkles className="w-3.5 h-3.5 text-amber-400" />
                <span className="text-slate-400">Style:</span>
                <span className="font-semibold">{activePhrase.style}</span>
              </div>

              {/* Layout Pill */}
              <div className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs bg-slate-900 border border-slate-700/70 text-slate-300">
                <Compass className="w-3.5 h-3.5 text-indigo-400" />
                <span className="text-slate-400">Layout:</span>
                <span className="font-semibold">{activePhrase.layout}</span>
              </div>
            </div>

            {/* Collapsed Presets & Styles Popover Trigger */}
            <div className="relative">
              <button
                onClick={() => setShowPresetDropdown(!showPresetDropdown)}
                className="px-3 py-1.5 rounded-lg text-xs font-medium bg-slate-900 hover:bg-slate-800 text-slate-200 border border-slate-700 flex items-center gap-1.5 shadow-sm transition"
              >
                <Wand2 className="w-3.5 h-3.5 text-cyan-400" />
                <span>Presets &amp; Styles</span>
                <ChevronDown className="w-3.5 h-3.5 text-slate-400" />
              </button>

              {showPresetDropdown && (
                <div className="absolute right-0 mt-1.5 w-64 rounded-xl bg-slate-900/95 backdrop-blur-md border border-slate-700 shadow-2xl p-2 z-50 space-y-1">
                  <div className="text-[10px] font-semibold uppercase tracking-wider text-slate-400 px-2 py-1">
                    Global Kinetic Look
                  </div>
                  {GLOBAL_PRESETS.map((preset) => (
                    <button
                      key={preset.id}
                      onClick={() => applyPresetToPhrase(preset.name)}
                      className={`w-full text-left px-2.5 py-2 rounded-lg transition flex items-start justify-between ${
                        activePhrase.style === preset.name
                          ? 'bg-cyan-950/70 border border-cyan-500/40 text-cyan-200'
                          : 'hover:bg-slate-800/80 text-slate-200'
                      }`}
                    >
                      <div>
                        <div className="text-xs font-semibold">{preset.name}</div>
                        <div className="text-[10px] text-slate-400">{preset.desc}</div>
                      </div>
                      {activePhrase.style === preset.name && (
                        <Check className="w-3.5 h-3.5 text-cyan-400 shrink-0 mt-0.5" />
                      )}
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>

          {/* -----------------------------------------------------------------
              CENTER MONITOR: WYSIWYG KINETIC MOTION GRAPHICS CANVAS
          ------------------------------------------------------------------ */}
          <div className="my-2 flex flex-col items-center justify-center shrink-0">
            <div
              ref={stageRef}
              onPointerDown={handleStagePointerDown}
              className={`relative w-full max-w-3xl aspect-[16/9] max-h-[340px] rounded-2xl overflow-hidden border transition-colors ${
                isDrawingMode
                  ? 'border-fuchsia-400/90 cursor-crosshair shadow-[0_0_35px_rgba(217,70,239,0.25)]'
                  : 'border-slate-700/80 shadow-[0_12px_45px_rgba(0,0,0,0.75)]'
              } bg-gradient-to-br ${activePhrase.bgGradient}`}
            >
              {/* Animated Stylized Phoenix / Fire + Complementary Scene FX Backdrop SVG */}
              <svg
                viewBox="0 0 800 450"
                className="absolute inset-0 w-full h-full object-cover pointer-events-none opacity-90"
              >
                <defs>
                  <radialGradient id="phoenixGlow" cx="55%" cy="48%" r="50%">
                    <stop offset="0%" stopColor="#fb923c" stopOpacity="0.55" />
                    <stop offset="45%" stopColor="#ea580c" stopOpacity="0.25" />
                    <stop offset="100%" stopColor="#090d14" stopOpacity="0.9" />
                  </radialGradient>
                  <linearGradient id="wingGrad" x1="0%" y1="100%" x2="100%" y2="0%">
                    <stop offset="0%" stopColor="#7c2d12" stopOpacity="0.6" />
                    <stop offset="50%" stopColor="#f97316" stopOpacity="0.85" />
                    <stop offset="100%" stopColor="#fde047" stopOpacity="0.95" />
                  </linearGradient>
                  <filter id="neonGlowFilter" x="-30%" y="-30%" width="160%" height="160%">
                    <feGaussianBlur stdDeviation="4" result="blur" />
                    <feMerge>
                      <feMergeNode in="blur" />
                      <feMergeNode in="SourceGraphic" />
                    </feMerge>
                  </filter>
                </defs>
                {/* Core glow */}
                <rect width="800" height="450" fill="url(#phoenixGlow)" />

                {/* COMPLEMENTARY BEAT-REACTIVE SCENE FX (Shockwave Rings / Wing Flare / Horizon Pulse) */}
                {activePhrase.sceneFx?.beatReactive === 'Shockwave Ring' && (
                  <g opacity={0.45 + Math.sin(playhead * Math.PI) * 0.35}>
                    <circle
                      cx="425"
                      cy="245"
                      r={70 + playhead * 190}
                      fill="none"
                      stroke="#fb923c"
                      strokeWidth="2.5"
                      strokeDasharray="12 8"
                    />
                    <circle
                      cx="425"
                      cy="245"
                      r={40 + playhead * 135}
                      fill="none"
                      stroke="#38bdf8"
                      strokeWidth="1.5"
                    />
                  </g>
                )}

                {activePhrase.sceneFx?.beatReactive === 'Starlight Ripple' && (
                  <g opacity="0.55">
                    <ellipse
                      cx="400"
                      cy="230"
                      rx={120 + playhead * 140}
                      ry={45 + playhead * 60}
                      fill="none"
                      stroke="#c084fc"
                      strokeWidth="2"
                      strokeDasharray="6 6"
                    />
                  </g>
                )}

                {activePhrase.sceneFx?.beatReactive === 'Horizon Pulse' && (
                  <line
                    x1="60"
                    y1={320 - Math.sin(playhead * Math.PI * 2) * 12}
                    x2="740"
                    y2={320 - Math.sin(playhead * Math.PI * 2) * 12}
                    stroke="#22d3ee"
                    strokeWidth="3"
                    opacity="0.5"
                    filter="url(#neonGlowFilter)"
                  />
                )}

                {/* Stylized Dramatic Phoenix Wings Silhouette (Pulses with Wing Flare) */}
                <g
                  transform={`translate(430, 255) scale(${
                    activePhrase.sceneFx?.beatReactive === 'Wing Flare'
                      ? 1 + Math.sin(playhead * Math.PI * 2) * 0.08
                      : 1
                  }) translate(-430, -255)`}
                >
                  <path
                    d="M420,250 C330,190 250,90 190,40 C240,130 280,200 370,260 Z"
                    fill="url(#wingGrad)"
                    opacity="0.65"
                  />
                  <path
                    d="M440,250 C520,180 610,80 680,30 C630,135 570,210 470,265 Z"
                    fill="url(#wingGrad)"
                    opacity="0.75"
                  />
                  <path
                    d="M425,265 C350,230 270,160 210,120 C275,200 340,255 415,285 Z"
                    fill="#fb923c"
                    opacity="0.45"
                  />
                  <path
                    d="M445,265 C525,220 605,155 660,115 C600,200 535,255 455,285 Z"
                    fill="#fde047"
                    opacity="0.5"
                  />
                  {/* Phoenix Head & Crest */}
                  <path
                    d="M415,310 C425,240 435,190 455,150 C465,145 475,150 470,160 C455,195 450,250 440,315 Z"
                    fill="#fde047"
                    opacity="0.7"
                  />
                </g>

                {/* COMPLEMENTARY ATMOSPHERIC PARTICLE FIELD (Moves with playhead) */}
                {[
                  { cx: 160, cy: 360, r: 3.5, c: '#fde047', speed: 90 },
                  { cx: 240, cy: 340, r: 3, c: '#fb923c', speed: 120 },
                  { cx: 580, cy: 330, r: 4, c: '#fb923c', speed: 105 },
                  { cx: 650, cy: 250, r: 2.5, c: '#fef08a', speed: 80 },
                  { cx: 310, cy: 190, r: 3.5, c: '#38bdf8', speed: 110 },
                  { cx: 490, cy: 380, r: 3, c: '#22d3ee', speed: 130 }
                ].map((pt, i) => {
                  const yOffset = (playhead * pt.speed) % 180;
                  const xWobble = Math.sin(playhead * 8 + i) * 14;
                  return (
                    <circle
                      key={i}
                      cx={pt.cx + xWobble}
                      cy={pt.cy - yOffset}
                      r={pt.r * (activePhrase.sceneFx?.intensity || 0.8)}
                      fill={pt.c}
                      opacity="0.75"
                    />
                  );
                })}
              </svg>

              {/* ===============================================================
                  COMPLEMENTARY DRAWN MOTION OVERLAYS (Trim-Paths, Boils, Waves)
              ================================================================ */}
              <svg
                viewBox="0 0 800 450"
                className="absolute inset-0 w-full h-full z-15 pointer-events-none"
              >
                {(activePhrase.drawings || []).map((drawing, dIdx) => {
                  const isSelected =
                    inspectorMode === 'motion-drawings' && drawing.id === selectedDrawing?.id;

                  // Calculate normalized progress within the drawing's start/end window
                  const span = Math.max(0.1, (drawing.endAt ?? 1) - (drawing.startAt ?? 0));
                  const localProgress = Math.max(
                    0,
                    Math.min(1, (playhead - (drawing.startAt ?? 0)) / span)
                  );

                  // Behavior calculations
                  const isTrim = drawing.behavior === 'Trim Draw-On';
                  const isBoil = drawing.behavior === 'Pulse & Boil';
                  const isWave = drawing.behavior === 'Wave Flow';

                  const dashArray = isTrim ? 1000 : isWave ? '18 10' : 'none';
                  const dashOffset = isTrim
                    ? 1000 * (1 - localProgress)
                    : isWave
                    ? -playhead * 260
                    : 0;

                  const boilJitterY = isBoil ? Math.sin(playhead * 32 + dIdx) * 3.5 : 0;
                  const boilScale = isBoil ? 1 + Math.sin(playhead * 16) * 0.025 : 1;

                  return (
                    <g
                      key={drawing.id}
                      transform={`translate(0, ${boilJitterY}) scale(${boilScale})`}
                      className="pointer-events-auto cursor-pointer"
                      onClick={(e) => {
                        e.stopPropagation();
                        setSelectedDrawingId(drawing.id);
                        setInspectorMode('motion-drawings');
                      }}
                    >
                      {/* Outer Halo Glow when Selected */}
                      {isSelected && (
                        <path
                          d={drawing.d}
                          fill="none"
                          stroke={drawing.color}
                          strokeWidth={(drawing.strokeWidth || 4) + 6}
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          opacity="0.28"
                          pathLength="1000"
                          strokeDasharray={dashArray}
                          strokeDashoffset={dashOffset}
                        />
                      )}

                      {/* Main Complementary Motion Drawing Path */}
                      <path
                        d={drawing.d}
                        fill="none"
                        stroke={drawing.color}
                        strokeWidth={drawing.strokeWidth || 4}
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        pathLength="1000"
                        strokeDasharray={dashArray}
                        strokeDashoffset={dashOffset}
                        style={{
                          filter: `drop-shadow(0 0 8px ${drawing.color})`
                        }}
                      />
                    </g>
                  );
                })}

                {/* Live Freehand Sketch Path in Progress */}
                {currentStrokePoints.length > 1 && (
                  <path
                    d={currentStrokePoints
                      .map((pt, i) => `${i === 0 ? 'M' : 'L'} ${pt.x} ${pt.y}`)
                      .join(' ')}
                    fill="none"
                    stroke={brushColor}
                    strokeWidth="4.5"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    style={{ filter: `drop-shadow(0 0 10px ${brushColor})` }}
                  />
                )}
              </svg>

              {/* Top Floating Canvas Mode Pills (Kinetic Motion / Draw Motion Overlay / Warp Path) */}
              <div className="absolute top-3 left-1/2 -translate-x-1/2 flex items-center gap-2 z-30">
                <button
                  onClick={() => {
                    setIsDrawingMode(false);
                    setInspectorMode('word');
                  }}
                  className={`px-3 py-1 rounded-full text-xs font-medium backdrop-blur-md border transition flex items-center gap-1.5 ${
                    !isDrawingMode && inspectorMode === 'word'
                      ? 'bg-slate-950/85 border-cyan-400/70 text-cyan-200 shadow-[0_0_12px_rgba(34,211,238,0.25)]'
                      : 'bg-slate-950/60 border-white/15 text-slate-300 hover:text-white'
                  }`}
                >
                  <Move className="w-3 h-3 text-cyan-400" />
                  <span>Word Transform</span>
                </button>

                <button
                  onClick={() => {
                    const next = !isDrawingMode;
                    setIsDrawingMode(next);
                    if (next) setInspectorMode('motion-drawings');
                  }}
                  className={`px-3 py-1 rounded-full text-xs font-medium backdrop-blur-md border transition flex items-center gap-1.5 ${
                    isDrawingMode
                      ? 'bg-fuchsia-950/90 border-fuchsia-400 text-fuchsia-100 shadow-[0_0_16px_rgba(217,70,239,0.45)]'
                      : 'bg-slate-950/75 border-fuchsia-500/40 text-fuchsia-200 hover:border-fuchsia-400'
                  }`}
                >
                  <PenTool className="w-3 h-3 text-fuchsia-400" />
                  <span>{isDrawingMode ? 'Sketching Motion Path...' : 'Draw Motion Overlay'}</span>
                </button>

                <button
                  onClick={() => setShowWarpPaths(!showWarpPaths)}
                  className={`px-3 py-1 rounded-full text-xs font-medium backdrop-blur-md border transition flex items-center gap-1.5 ${
                    showWarpPaths
                      ? 'bg-slate-950/80 border-amber-400/60 text-amber-200'
                      : 'bg-slate-950/60 border-white/15 text-slate-300'
                  }`}
                >
                  <Spline className="w-3 h-3 text-amber-400" />
                  <span>Trajectories</span>
                </button>
              </div>

              {/* Floating Quick-Stamp & Color Palette Bar when Drawing Mode is Active */}
              {isDrawingMode && (
                <div
                  onPointerDown={(e) => e.stopPropagation()}
                  className="absolute top-12 left-1/2 -translate-x-1/2 flex items-center gap-2 px-3 py-1.5 rounded-full bg-slate-950/90 backdrop-blur-md border border-fuchsia-500/50 z-30 shadow-xl"
                >
                  <span className="text-[10px] font-semibold uppercase tracking-wider text-fuchsia-300 mr-1">
                    Ink:
                  </span>
                  {['#22d3ee', '#fb923c', '#fde047', '#f43f5e', '#c084fc'].map((c) => (
                    <button
                      key={c}
                      onClick={() => setBrushColor(c)}
                      className={`w-4 h-4 rounded-full transition transform ${
                        brushColor === c ? 'scale-125 ring-2 ring-white' : 'opacity-75 hover:opacity-100'
                      }`}
                      style={{ backgroundColor: c }}
                    />
                  ))}
                  <div className="h-3 w-[1px] bg-slate-700 mx-1" />
                  <span className="text-[10px] text-slate-400">
                    Click &amp; drag anywhere on scene to draw an animated trim-path
                  </span>
                </div>
              )}

              {/* Inter-Word Kinetic Flow Arrows (Inside Canvas) */}
              {showWarpPaths && (
                <svg className="absolute inset-0 w-full h-full pointer-events-none z-10">
                  <defs>
                    <marker
                      id="kineticArrow"
                      markerWidth="6"
                      markerHeight="6"
                      refX="4"
                      refY="3"
                      orient="auto"
                    >
                      <path d="M0,0 L6,3 L0,6 Z" fill="#22d3ee" />
                    </marker>
                  </defs>
                  {activePhrase.words.slice(0, -1).map((w, i) => {
                    const next = activePhrase.words[i + 1];
                    return (
                      <line
                        key={w.id}
                        x1={`${w.x + 5}%`}
                        y1={`${w.y}%`}
                        x2={`${next.x - 6}%`}
                        y2={`${next.y}%`}
                        stroke="#22d3ee"
                        strokeWidth="1.75"
                        strokeDasharray="3 3"
                        markerEnd="url(#kineticArrow)"
                        opacity="0.75"
                      />
                    );
                  })}
                </svg>
              )}

              {/* Interactive Kinetic Words with Transform Handles */}
              {activePhrase.words.map((word, idx) => {
                const isWordSelected = word.id === selectedWord?.id;
                // Subtle live animation pulse when playing
                const liveBounce = isPlaying
                  ? Math.sin((playhead * 12 + idx) * 1.6) * 5
                  : 0;
                const isFire = word.style === 'Fire' || word.effect === 'Impact';

                return (
                  <div
                    key={word.id}
                    ref={(el) => (wordRefs.current[word.id] = el)}
                    onPointerDown={(e) => {
                      e.stopPropagation();
                      setSelectedWordId(word.id);
                      setDraggingWordId(word.id);
                    }}
                    style={{
                      left: `${word.x}%`,
                      top: `${word.y}%`,
                      transform: `translate(-50%, calc(-50% + ${liveBounce}px)) rotate(${word.rotate}deg) scale(${word.scale})`
                    }}
                    className={`absolute z-20 cursor-grab active:cursor-grabbing select-none transition-shadow ${
                      isWordSelected ? 'z-30' : 'hover:opacity-95'
                    }`}
                  >
                    {/* Transform Bounding Box + Corner Handles */}
                    <div
                      className={`relative px-2.5 py-0.5 rounded ${
                        isWordSelected
                          ? 'border border-cyan-300/90 bg-cyan-500/5 shadow-[0_0_20px_rgba(34,211,238,0.25)]'
                          : 'border border-white/25 hover:border-white/60'
                      }`}
                    >
                      {/* Top Rotation Stem Handle */}
                      {isWordSelected && (
                        <div className="absolute -top-4 left-1/2 -translate-x-1/2 flex flex-col items-center">
                          <div className="w-2 h-2 rounded-full bg-cyan-300 border border-slate-950 shadow" />
                          <div className="w-[1px] h-2 bg-cyan-300" />
                        </div>
                      )}

                      {/* 4 Corner Anchor Squares */}
                      {isWordSelected && (
                        <>
                          <span className="absolute -top-1 -left-1 w-2 h-2 bg-white border border-slate-900" />
                          <span className="absolute -top-1 -right-1 w-2 h-2 bg-white border border-slate-900" />
                          <span className="absolute -bottom-1 -left-1 w-2 h-2 bg-white border border-slate-900" />
                          <span className="absolute -bottom-1 -right-1 w-2 h-2 bg-white border border-slate-900" />
                        </>
                      )}

                      {/* Motion-Blur Ghost Echo if Zoom/Motion Blur */}
                      {(word.effect === 'Zoom' || word.style === 'Motion Blur') && (
                        <span
                          aria-hidden="true"
                          className="absolute left-6 top-0 opacity-30 blur-[2px] pointer-events-none font-black italic tracking-tight"
                          style={{ color: word.color }}
                        >
                          {word.text}
                        </span>
                      )}

                      {/* Main Kinetic Typography Text */}
                      <span
                        className={`font-black tracking-tight block leading-none ${
                          isFire
                            ? 'bg-gradient-to-b from-yellow-200 via-amber-400 to-orange-600 bg-clip-text text-transparent drop-shadow-[0_4px_16px_rgba(249,115,22,0.85)]'
                            : 'drop-shadow-[0_2px_10px_rgba(0,0,0,0.85)]'
                        }`}
                        style={!isFire ? { color: word.color, fontSize: '22px' } : { fontSize: '26px' }}
                      >
                        {word.text}
                      </span>
                    </div>
                  </div>
                );
              })}

              {/* Drag Hint in Bottom Left of Canvas */}
              <div className="absolute bottom-2.5 left-3 right-3 flex items-center justify-between pointer-events-none">
                <div className="px-2 py-0.5 rounded bg-black/55 backdrop-blur-sm text-[10px] text-slate-300 border border-white/10">
                  {isDrawingMode
                    ? 'Sketching Mode: Draw complementary motion lines around the lyrics'
                    : 'Drag words to position • Click any drawn neon path or token below to edit motion'}
                </div>
                <div className="px-2 py-0.5 rounded bg-black/60 backdrop-blur-sm text-[10px] text-amber-300 border border-amber-500/30 flex items-center gap-1.5">
                  <Sparkles className="w-3 h-3 text-amber-400" />
                  <span>Scene FX: {activePhrase.sceneFx?.atmosphere} + {activePhrase.sceneFx?.beatReactive}</span>
                </div>
              </div>
            </div>

            {/* Video Playback & Scrubber Bar */}
            <div className="w-full max-w-3xl mt-2.5 px-3 py-1.5 rounded-xl bg-slate-900/90 border border-slate-800/90 flex items-center gap-3">
              <button
                onClick={() => setIsPlaying(!isPlaying)}
                className="w-7 h-7 rounded-lg bg-cyan-500 hover:bg-cyan-400 text-slate-950 flex items-center justify-center transition shadow-sm shadow-cyan-500/30"
              >
                {isPlaying ? <Pause className="w-3.5 h-3.5 fill-current" /> : <Play className="w-3.5 h-3.5 fill-current ml-0.5" />}
              </button>

              {/* Interactive Timeline Scrubber */}
              <div
                onClick={(e) => {
                  const rect = e.currentTarget.getBoundingClientRect();
                  const pos = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
                  setPlayhead(+pos.toFixed(2));
                }}
                className="flex-1 h-2 bg-slate-800 rounded-full relative cursor-pointer flex items-center"
              >
                <div
                  className="h-full bg-gradient-to-r from-cyan-400 to-teal-300 rounded-full"
                  style={{ width: `${playhead * 100}%` }}
                />
                <div
                  className="w-3.5 h-3.5 rounded-full bg-white border-2 border-cyan-400 shadow-[0_0_10px_#22d3ee] absolute -translate-x-1/2"
                  style={{ left: `${playhead * 100}%` }}
                />
                {/* Right Tether Anchor Dot on Scrubber */}
                <div
                  ref={scrubberDotRef}
                  className="w-2.5 h-2.5 rounded-full bg-cyan-400 shadow-[0_0_8px_#22d3ee] absolute right-6"
                  title="Synced to Vertical Audio Stem"
                />
              </div>

              <div className="text-xs font-mono text-slate-300 tabular-nums">
                0:{currentTimestamp.padStart(4, '0')} <span className="text-slate-500">/ 3:15</span>
              </div>

              <button
                onClick={() => setPlayhead(0)}
                className="p-1 text-slate-400 hover:text-white rounded"
                title="Replay Phrase"
              >
                <RotateCcw className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>

          {/* -----------------------------------------------------------------
              UNIFIED INTERACTIVE TOKEN BAR: WORDS + COMPLEMENTARY MOTION DRAWINGS
          ------------------------------------------------------------------ */}
          <div className="my-1.5 flex items-center justify-between flex-wrap gap-2 py-2 px-3 rounded-xl bg-[#0c121d] border border-slate-800/80 shrink-0">
            {/* Left Group: Kinetic Lyric Word Tokens */}
            <div className="flex items-center flex-wrap gap-1.5">
              <span className="text-[10px] font-mono uppercase tracking-wider text-slate-500 mr-1">
                Lyrics [
              </span>
              {activePhrase.words.map((word) => {
                const active = inspectorMode === 'word' && word.id === selectedWord?.id;
                const isImpact = word.effect === 'Impact' || word.style === 'Fire';

                return (
                  <button
                    key={word.id}
                    onClick={() => {
                      setSelectedWordId(word.id);
                      setInspectorMode('word');
                      setIsDrawingMode(false);
                    }}
                    className={`px-2.5 py-1 rounded-lg text-xs font-semibold transition-all flex items-center gap-1.5 border ${
                      active
                        ? isImpact
                          ? 'bg-amber-950/90 border-amber-400 text-amber-100 shadow-[0_0_12px_rgba(251,146,60,0.3)] scale-105'
                          : 'bg-cyan-950/90 border-cyan-400 text-cyan-100 shadow-[0_0_12px_rgba(34,211,238,0.28)] scale-105'
                        : 'bg-slate-900/90 border-slate-800 text-slate-300 hover:border-slate-600'
                    }`}
                  >
                    <span>{word.text}</span>
                    <span className="text-[10px] opacity-75 font-normal">{word.effect}</span>
                  </button>
                );
              })}
              <span className="text-[10px] font-mono text-slate-500 ml-0.5">]</span>
            </div>

            {/* Right Group: Complementary Motion Drawings & Scene Overlays */}
            <div className="flex items-center flex-wrap gap-1.5">
              <span className="text-[10px] font-mono uppercase tracking-wider text-fuchsia-400/90 mr-1 flex items-center gap-1">
                <PenTool className="w-3 h-3" />
                Motion Drawings [
              </span>
              {(activePhrase.drawings || []).map((drawing) => {
                const active =
                  inspectorMode === 'motion-drawings' && drawing.id === selectedDrawing?.id;
                return (
                  <button
                    key={drawing.id}
                    onClick={() => {
                      setSelectedDrawingId(drawing.id);
                      setInspectorMode('motion-drawings');
                    }}
                    className={`px-2.5 py-1 rounded-lg text-xs font-semibold transition-all flex items-center gap-1.5 border ${
                      active
                        ? 'bg-fuchsia-950/90 border-fuchsia-400 text-fuchsia-100 shadow-[0_0_14px_rgba(217,70,239,0.35)] scale-105'
                        : 'bg-slate-900/90 border-slate-800 text-slate-300 hover:border-fuchsia-500/50'
                    }`}
                  >
                    <span
                      className="w-2 h-2 rounded-full shrink-0"
                      style={{ backgroundColor: drawing.color }}
                    />
                    <span>{drawing.name}</span>
                    <span className="text-[10px] px-1.5 py-0.2 rounded bg-fuchsia-500/20 text-fuchsia-300">
                      {drawing.behavior === 'Trim Draw-On'
                        ? '↝ Trim'
                        : drawing.behavior === 'Pulse & Boil'
                        ? '⚡ Boil'
                        : '〰️ Wave'}
                    </span>
                  </button>
                );
              })}
              <button
                onClick={() => {
                  setInspectorMode('motion-drawings');
                  setIsDrawingMode(true);
                }}
                className="px-2 py-1 rounded-lg text-xs font-medium bg-fuchsia-950/50 hover:bg-fuchsia-900/70 text-fuchsia-300 border border-fuchsia-500/40 flex items-center gap-1 transition"
                title="Sketch a new motion drawing on the stage"
              >
                <Plus className="w-3 h-3" />
                <span>Sketch</span>
              </button>
              <span className="text-[10px] font-mono text-fuchsia-400/90 ml-0.5">]</span>
            </div>
          </div>

          {/* -----------------------------------------------------------------
              BOTTOM DOCK: COMPLEMENTARY MOTION DRAWINGS & SCENE FX vs WORD ENGINE
          ------------------------------------------------------------------ */}
          <div className="bg-[#0c111b] border border-slate-800/90 rounded-2xl p-3.5 shrink-0">
            {/* Dock Mode Header & Switcher */}
            <div className="flex items-center justify-between mb-2.5">
              <div className="flex items-center gap-2">
                <div className="flex items-center bg-slate-900 p-0.5 rounded-lg border border-slate-800">
                  <button
                    onClick={() => setInspectorMode('motion-drawings')}
                    className={`px-2.5 py-1 rounded-md text-xs font-bold transition flex items-center gap-1.5 ${
                      inspectorMode === 'motion-drawings'
                        ? 'bg-fuchsia-950/90 text-fuchsia-200 border border-fuchsia-500/50 shadow-sm'
                        : 'text-slate-400 hover:text-slate-200'
                    }`}
                  >
                    <PenTool className="w-3.5 h-3.5 text-fuchsia-400" />
                    <span>Complementary Drawings &amp; Scene FX</span>
                  </button>
                  <button
                    onClick={() => {
                      setInspectorMode('word');
                      setIsDrawingMode(false);
                    }}
                    className={`px-2.5 py-1 rounded-md text-xs font-bold transition flex items-center gap-1.5 ${
                      inspectorMode === 'word'
                        ? 'bg-cyan-950/90 text-cyan-200 border border-cyan-500/50 shadow-sm'
                        : 'text-slate-400 hover:text-slate-200'
                    }`}
                  >
                    <Sliders className="w-3.5 h-3.5 text-cyan-400" />
                    <span>Word Choreography ("{selectedWord?.text}")</span>
                  </button>
                </div>
              </div>

              {inspectorMode === 'motion-drawings' ? (
                <div className="flex items-center gap-3 text-xs">
                  {selectedDrawing && (
                    <>
                      <label className="flex items-center gap-1.5 text-slate-400">
                        <span>Draw Window</span>
                        <input
                          type="range"
                          min="0.2"
                          max="1.0"
                          step="0.05"
                          value={selectedDrawing.endAt || 0.9}
                          onChange={(e) =>
                            updateSelectedDrawingProp('endAt', parseFloat(e.target.value))
                          }
                          className="w-20 accent-fuchsia-400 cursor-pointer"
                        />
                        <span className="font-mono text-[11px] text-fuchsia-300">
                          {Math.round((selectedDrawing.endAt || 0.9) * 100)}%
                        </span>
                      </label>

                      <label className="flex items-center gap-1.5 text-slate-400">
                        <span>Stroke</span>
                        <input
                          type="range"
                          min="2"
                          max="9"
                          step="0.5"
                          value={selectedDrawing.strokeWidth || 4}
                          onChange={(e) =>
                            updateSelectedDrawingProp('strokeWidth', parseFloat(e.target.value))
                          }
                          className="w-16 accent-amber-400 cursor-pointer"
                        />
                      </label>

                      <button
                        onClick={deleteSelectedDrawing}
                        className="p-1 rounded-lg bg-rose-950/60 hover:bg-rose-900/80 text-rose-300 border border-rose-700/50 transition"
                        title="Remove Selected Drawing"
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </button>
                    </>
                  )}
                </div>
              ) : (
                /* Quick Scale & Rotation Sliders for Selected Word */
                <div className="flex items-center gap-4 text-xs">
                  <label className="flex items-center gap-2 text-slate-400">
                    <span>Scale</span>
                    <input
                      type="range"
                      min="0.7"
                      max="2.6"
                      step="0.05"
                      value={selectedWord?.scale || 1}
                      onChange={(e) => updateSelectedWordProp('scale', parseFloat(e.target.value))}
                      className="w-20 accent-cyan-400 cursor-pointer"
                    />
                    <span className="font-mono text-[11px] text-slate-200 w-8">
                      {selectedWord?.scale}x
                    </span>
                  </label>

                  <label className="flex items-center gap-2 text-slate-400">
                    <span>Tilt</span>
                    <input
                      type="range"
                      min="-25"
                      max="25"
                      step="1"
                      value={selectedWord?.rotate || 0}
                      onChange={(e) =>
                        updateSelectedWordProp('rotate', parseInt(e.target.value, 10))
                      }
                      className="w-20 accent-amber-400 cursor-pointer"
                    />
                    <span className="font-mono text-[11px] text-slate-200 w-8">
                      {selectedWord?.rotate}°
                    </span>
                  </label>
                </div>
              )}
            </div>

            {inspectorMode === 'motion-drawings' ? (
              /* ===============================================================
                 COMPLEMENTARY MOTION DRAWINGS & SCENE FX ENGINE (Non-Word Motion)
              ================================================================ */
              <div className="grid grid-cols-12 gap-4 items-stretch">
                {/* Column 1: Drawn Path Motion Behavior & Brush Shader (4 cols) */}
                <div className="col-span-4 flex flex-col justify-between space-y-2">
                  <div className="flex items-center justify-between">
                    <span className="text-[11px] font-medium text-slate-400">
                      Drawing Motion Behavior ({selectedDrawing?.name || 'None'})
                    </span>
                  </div>
                  <div className="grid grid-cols-3 gap-1.5">
                    {DRAWING_BEHAVIORS.map((beh) => {
                      const active = selectedDrawing?.behavior === beh.id;
                      return (
                        <button
                          key={beh.id}
                          onClick={() => updateSelectedDrawingProp('behavior', beh.id)}
                          className={`p-2 rounded-xl border text-center flex flex-col items-center justify-center gap-1 transition ${
                            active
                              ? 'bg-fuchsia-950/80 border-fuchsia-400 text-fuchsia-100 shadow-[0_0_12px_rgba(217,70,239,0.25)]'
                              : 'bg-slate-900/70 border-slate-800 text-slate-400 hover:border-slate-700 hover:text-slate-200'
                          }`}
                        >
                          <Activity
                            className={`w-3.5 h-3.5 ${
                              active ? 'text-fuchsia-400' : 'text-slate-500'
                            }`}
                          />
                          <span className="text-[10px] font-semibold leading-tight">
                            {beh.label}
                          </span>
                        </button>
                      );
                    })}
                  </div>

                  {/* Brush Shader Style */}
                  <div className="grid grid-cols-4 gap-1.5 pt-0.5">
                    {DRAWING_BRUSH_STYLES.map((b) => {
                      const active = selectedDrawing?.brushStyle === b.id;
                      return (
                        <button
                          key={b.id}
                          onClick={() => {
                            updateSelectedDrawingProp('brushStyle', b.id);
                            updateSelectedDrawingProp('color', b.color);
                          }}
                          className={`px-1.5 py-1 rounded-lg border text-center flex items-center justify-center gap-1 transition ${
                            active
                              ? 'bg-amber-950/80 border-amber-400 text-amber-200'
                              : 'bg-slate-900/60 border-slate-800 text-slate-400 hover:text-slate-200'
                          }`}
                        >
                          <span className="text-[10px]">{b.badge}</span>
                          <span className="text-[9px] font-semibold truncate">{b.label}</span>
                        </button>
                      );
                    })}
                  </div>
                </div>

                {/* Column 2: Quick Motion Doodle Stamps (4 cols) */}
                <div className="col-span-4 flex flex-col justify-between">
                  <span className="text-[11px] font-medium text-slate-400 mb-1.5">
                    Add Complementary Motion Doodle Stamp
                  </span>
                  <div className="grid grid-cols-2 gap-1.5 flex-1">
                    {MOTION_DOODLE_STAMPS.map((stamp, idx) => (
                      <button
                        key={idx}
                        onClick={() => addDoodleStamp(stamp)}
                        className="px-2.5 py-1.5 rounded-xl border border-slate-800 bg-slate-900/70 hover:bg-slate-800/80 hover:border-fuchsia-500/50 text-left flex flex-col justify-between transition group"
                      >
                        <div className="flex items-center justify-between">
                          <span className="text-[10px] font-semibold text-slate-200 group-hover:text-fuchsia-200 truncate">
                            + {stamp.name}
                          </span>
                          <span
                            className="w-2 h-2 rounded-full"
                            style={{ backgroundColor: stamp.color }}
                          />
                        </div>
                        <svg viewBox="100 80 550 360" className="w-full h-5 opacity-85">
                          <path
                            d={stamp.d}
                            fill="none"
                            stroke={stamp.color}
                            strokeWidth="14"
                            strokeLinecap="round"
                          />
                        </svg>
                      </button>
                    ))}
                  </div>
                </div>

                {/* Column 3: Ambient Scene & Beat-Reactive Motion FX (4 cols) */}
                <div className="col-span-4 flex flex-col justify-between bg-slate-950/90 border border-slate-800/90 rounded-xl p-2.5">
                  <div className="flex items-center justify-between">
                    <span className="text-[11px] font-semibold text-amber-300 flex items-center gap-1">
                      <Sparkles className="w-3.5 h-3.5 text-amber-400" />
                      Scene &amp; Beat Motion FX
                    </span>
                    <span className="text-[10px] font-mono text-slate-400">
                      Complements Lyrics
                    </span>
                  </div>

                  <div className="space-y-1.5 my-1">
                    <div className="flex items-center justify-between text-[10px] text-slate-400">
                      <span>Atmosphere Particles:</span>
                      <div className="flex gap-1">
                        {SCENE_ATMOSPHERES.slice(0, 3).map((atm) => (
                          <button
                            key={atm}
                            onClick={() => updateSceneFxProp('atmosphere', atm)}
                            className={`px-1.5 py-0.5 rounded border text-[9px] font-medium transition ${
                              activePhrase.sceneFx?.atmosphere === atm
                                ? 'bg-amber-950/90 border-amber-400 text-amber-200'
                                : 'bg-slate-900 border-slate-800 text-slate-400 hover:text-slate-200'
                            }`}
                          >
                            {atm}
                          </button>
                        ))}
                      </div>
                    </div>

                    <div className="flex items-center justify-between text-[10px] text-slate-400">
                      <span>Beat Reactive Pulse:</span>
                      <div className="flex gap-1">
                        {BEAT_REACTIVE_FX.slice(0, 3).map((fx) => (
                          <button
                            key={fx}
                            onClick={() => updateSceneFxProp('beatReactive', fx)}
                            className={`px-1.5 py-0.5 rounded border text-[9px] font-medium transition ${
                              activePhrase.sceneFx?.beatReactive === fx
                                ? 'bg-cyan-950/90 border-cyan-400 text-cyan-200'
                                : 'bg-slate-900 border-slate-800 text-slate-400 hover:text-slate-200'
                            }`}
                          >
                            {fx}
                          </button>
                        ))}
                      </div>
                    </div>
                  </div>

                  <label className="flex items-center justify-between text-[10px] text-slate-400 pt-1 border-t border-slate-800/80">
                    <span>Ambient FX Intensity</span>
                    <input
                      type="range"
                      min="0.2"
                      max="1.4"
                      step="0.05"
                      value={activePhrase.sceneFx?.intensity || 0.85}
                      onChange={(e) =>
                        updateSceneFxProp('intensity', parseFloat(e.target.value))
                      }
                      className="w-28 accent-amber-400 cursor-pointer"
                    />
                  </label>
                </div>
              </div>
            ) : (
              /* ===============================================================
                 WORD CHOREOGRAPHY + EASING CURVE ENGINE
              ================================================================ */
              <div className="grid grid-cols-12 gap-4 items-stretch">
              {/* Column 1: Choreography (3 cols) */}
              <div className="col-span-4 flex flex-col justify-between">
                <span className="text-[11px] font-medium text-slate-400 mb-1.5">
                  Choreography
                </span>
                <div className="grid grid-cols-3 gap-2 flex-1">
                  {CHOREOGRAPHY_OPTIONS.map((item) => {
                    const active = selectedWord?.choreography === item.id;
                    return (
                      <button
                        key={item.id}
                        onClick={() => updateSelectedWordProp('choreography', item.id)}
                        className={`p-2 rounded-xl border text-center flex flex-col items-center justify-center gap-1 transition ${
                          active
                            ? 'bg-cyan-950/70 border-cyan-400 text-cyan-200 shadow-[0_0_12px_rgba(34,211,238,0.2)]'
                            : 'bg-slate-900/70 border-slate-800 text-slate-400 hover:border-slate-700 hover:text-slate-200'
                        }`}
                      >
                        <Compass className={`w-4 h-4 ${active ? 'text-cyan-400' : 'text-slate-500'}`} />
                        <span className="text-[11px] font-semibold leading-tight">{item.label}</span>
                      </button>
                    );
                  })}
                </div>
              </div>

              {/* Column 2: Motion Styles (4 cols) */}
              <div className="col-span-4 flex flex-col justify-between">
                <span className="text-[11px] font-medium text-slate-400 mb-1.5">
                  Motion Styles &amp; Shaders
                </span>
                <div className="grid grid-cols-3 gap-1.5 flex-1">
                  {MOTION_STYLES.map((style) => {
                    const active = selectedWord?.style === style.id;
                    return (
                      <button
                        key={style.id}
                        onClick={() => updateSelectedWordProp('style', style.id)}
                        className={`px-2 py-1.5 rounded-xl border text-center flex flex-col items-center justify-center gap-0.5 transition ${
                          active
                            ? 'bg-amber-950/70 border-amber-400 text-amber-200 shadow-[0_0_12px_rgba(251,146,60,0.25)]'
                            : 'bg-slate-900/70 border-slate-800 text-slate-400 hover:border-slate-700 hover:text-slate-200'
                        }`}
                      >
                        <span className="text-xs">{style.badge}</span>
                        <span className="text-[10px] font-semibold leading-tight">{style.label}</span>
                      </button>
                    );
                  })}
                </div>
              </div>

              {/* Column 3: Interactive Easing Curve Graph Editor (4 cols) */}
              <div className="col-span-4 flex flex-col">
                <div className="flex items-center justify-between mb-1.5">
                  <span className="text-[11px] font-medium text-slate-400">
                    Easing Curve Graph Editor
                  </span>
                  <span className="text-[10px] font-mono text-cyan-400">
                    cubic-bezier({cp1.x}, {cp1.y}, {cp2.x}, {cp2.y})
                  </span>
                </div>

                <div className="flex-1 bg-slate-950 border border-slate-800 rounded-xl p-2 relative overflow-hidden">
                  <svg
                    ref={svgCurveRef}
                    viewBox="0 0 200 85"
                    className="w-full h-20 overflow-visible cursor-crosshair"
                  >
                    {/* Subtle Grid Lines */}
                    <line x1="0" y1="21" x2="200" y2="21" stroke="#1e293b" strokeWidth="1" />
                    <line x1="0" y1="42" x2="200" y2="42" stroke="#1e293b" strokeWidth="1" />
                    <line x1="0" y1="63" x2="200" y2="63" stroke="#1e293b" strokeWidth="1" />
                    <line x1="50" y1="0" x2="50" y2="85" stroke="#1e293b" strokeWidth="1" />
                    <line x1="100" y1="0" x2="100" y2="85" stroke="#1e293b" strokeWidth="1" />
                    <line x1="150" y1="0" x2="150" y2="85" stroke="#1e293b" strokeWidth="1" />

                    {/* Control Handle Guide Lines */}
                    <line
                      x1="10"
                      y1="78"
                      x2={cp1.x * 180 + 10}
                      y2={(1 - cp1.y) * 70 + 8}
                      stroke="#22d3ee"
                      strokeWidth="1.2"
                      strokeDasharray="2 2"
                    />
                    <line
                      x1="190"
                      y1="8"
                      x2={cp2.x * 180 + 10}
                      y2={(1 - cp2.y) * 70 + 8}
                      stroke="#f97316"
                      strokeWidth="1.2"
                      strokeDasharray="2 2"
                    />

                    {/* Area Fill Under Bezier */}
                    <path
                      d={`M 10 78 C ${cp1.x * 180 + 10} ${(1 - cp1.y) * 70 + 8}, ${
                        cp2.x * 180 + 10
                      } ${(1 - cp2.y) * 70 + 8}, 190 8 L 190 78 Z`}
                      fill="rgba(34, 211, 238, 0.12)"
                    />

                    {/* Main Bezier Curve */}
                    <path
                      d={`M 10 78 C ${cp1.x * 180 + 10} ${(1 - cp1.y) * 70 + 8}, ${
                        cp2.x * 180 + 10
                      } ${(1 - cp2.y) * 70 + 8}, 190 8`}
                      fill="none"
                      stroke="#22d3ee"
                      strokeWidth="2.5"
                    />

                    {/* Start / End Fixed Points */}
                    <circle cx="10" cy="78" r="3.5" fill="#0f172a" stroke="#22d3ee" strokeWidth="2" />
                    <circle cx="190" cy="8" r="3.5" fill="#0f172a" stroke="#22d3ee" strokeWidth="2" />

                    {/* Draggable Control Point 1 */}
                    <circle
                      cx={cp1.x * 180 + 10}
                      cy={(1 - cp1.y) * 70 + 8}
                      r="5.5"
                      fill="#22d3ee"
                      stroke="#ffffff"
                      strokeWidth="1.5"
                      className="cursor-grab active:cursor-grabbing"
                      onPointerDown={(e) => {
                        e.stopPropagation();
                        setDraggingHandle('cp1');
                      }}
                    />

                    {/* Draggable Control Point 2 */}
                    <circle
                      cx={cp2.x * 180 + 10}
                      cy={(1 - cp2.y) * 70 + 8}
                      r="5.5"
                      fill="#f97316"
                      stroke="#ffffff"
                      strokeWidth="1.5"
                      className="cursor-grab active:cursor-grabbing"
                      onPointerDown={(e) => {
                        e.stopPropagation();
                        setDraggingHandle('cp2');
                      }}
                    />
                  </svg>
                </div>
              </div>
            </div>
            )}
          </div>
        </main>

        {/* ===================================================================
            3. RIGHT PANEL: VERTICAL MULTI-TRACK KEYFRAME & AUDIO TIMELINE
        ==================================================================== */}
        <aside className="w-36 bg-[#070b11] border-l border-slate-800/80 flex flex-col justify-between p-2 shrink-0 z-20 relative">
          <div className="text-[10px] font-mono uppercase tracking-wider text-slate-400 text-center border-b border-slate-800 pb-1.5">
            Stem &amp; FX Sync
          </div>

          {/* Vertical Tracks Container (3 Columns: Audio Spine | Word Blocks | Drawn FX Blocks) */}
          <div className="flex-1 my-2 grid grid-cols-3 gap-1.5 relative">
            {/* Track 1: Main Phrase Spine with Tether Node */}
            <div className="relative bg-slate-900/70 border border-cyan-500/40 rounded-lg overflow-hidden flex flex-col justify-between py-3 items-center shadow-[inset_0_0_15px_rgba(34,211,238,0.12)]">
              <div className="w-full px-1 space-y-1">
                <div className="h-1 bg-cyan-500/40 rounded" />
                <div className="h-1 bg-cyan-500/20 rounded" />
              </div>

              {/* Target Anchor Node connected by Right Tether */}
              <div
                ref={rightTrackNodeRef}
                className="w-4 h-4 rounded-full bg-cyan-400 border-2 border-white shadow-[0_0_12px_#22d3ee] z-20"
                title="Active Phrase Sync Point"
              />

              {/* Simulated Waveform Bars */}
              <div className="w-full px-1.5 space-y-1.5 opacity-60">
                {[40, 85, 65, 95, 50, 80, 60, 90, 45, 70].map((w, i) => (
                  <div
                    key={i}
                    className="h-1 bg-cyan-400 rounded-full mx-auto"
                    style={{ width: `${w}%` }}
                  />
                ))}
              </div>

              <div className="text-[9px] font-mono text-amber-400 px-1">
                ↝ ⋅°
              </div>
            </div>

            {/* Track 2: Word Keyframe Blocks */}
            <div className="relative flex flex-col gap-1.5 py-3">
              {activePhrase.words.map((word, index) => {
                const isSelected = inspectorMode === 'word' && word.id === selectedWord?.id;
                return (
                  <div
                    key={word.id}
                    onClick={() => {
                      setSelectedWordId(word.id);
                      setInspectorMode('word');
                    }}
                    style={{ height: `${Math.max(14, 18 + index * 2)}%` }}
                    className={`w-full rounded-md border p-1 cursor-pointer transition flex flex-col justify-between overflow-hidden ${
                      isSelected
                        ? 'bg-cyan-950/90 border-cyan-400 shadow-[0_0_10px_rgba(34,211,238,0.3)]'
                        : 'bg-slate-900/80 border-slate-700/80 hover:border-slate-500'
                    }`}
                  >
                    <span className="text-[8px] font-bold text-slate-200 truncate">
                      {word.text}
                    </span>
                    <svg viewBox="0 0 40 20" className="w-full h-3.5 opacity-80">
                      <polyline
                        fill="none"
                        stroke={isSelected ? '#22d3ee' : '#64748b'}
                        strokeWidth="1.5"
                        points="2,16 14,6 26,12 38,3"
                      />
                    </svg>
                  </div>
                );
              })}
            </div>

            {/* Track 3: Complementary Motion Drawings & Scene FX Stem */}
            <div className="relative flex flex-col gap-2 py-2">
              {(activePhrase.drawings || []).map((drawing) => {
                const isSelected =
                  inspectorMode === 'motion-drawings' && drawing.id === selectedDrawing?.id;
                return (
                  <div
                    key={drawing.id}
                    onClick={() => {
                      setSelectedDrawingId(drawing.id);
                      setInspectorMode('motion-drawings');
                    }}
                    className={`flex-1 w-full rounded-md border p-1 cursor-pointer transition flex flex-col justify-between overflow-hidden ${
                      isSelected
                        ? 'bg-fuchsia-950/90 border-fuchsia-400 shadow-[0_0_12px_rgba(217,70,239,0.35)]'
                        : 'bg-slate-900/80 border-fuchsia-800/40 hover:border-fuchsia-500/60'
                    }`}
                  >
                    <span
                      className="text-[7px] font-bold truncate"
                      style={{ color: drawing.color }}
                    >
                      {drawing.name}
                    </span>
                    <svg viewBox="100 80 550 360" className="w-full h-5 opacity-90">
                      <path
                        d={drawing.d}
                        fill="none"
                        stroke={drawing.color}
                        strokeWidth="22"
                        strokeLinecap="round"
                      />
                    </svg>
                    <span className="text-[7px] font-mono text-slate-400 truncate">
                      {drawing.behavior.split(' ')[0]}
                    </span>
                  </div>
                );
              })}
            </div>
          </div>

          <div className="text-[9px] font-mono text-slate-500 text-center border-t border-slate-800 pt-1.5">
            60 FPS • GPU
          </div>
        </aside>
      </div>

      {/* =====================================================================
          TAURI & DEV HANDOFF SPECIFICATION MODAL
      ====================================================================== */}
      {showTauriModal && (
        <div className="fixed inset-0 bg-black/75 backdrop-blur-sm z-50 flex items-center justify-center p-4">
          <div className="bg-slate-900 border border-slate-700 rounded-2xl max-w-xl w-full p-5 shadow-2xl space-y-4">
            <div className="flex items-center justify-between border-b border-slate-800 pb-3">
              <div className="flex items-center gap-2">
                <Code className="w-5 h-5 text-cyan-400" />
                <h2 className="text-sm font-bold text-white">
                  Tauri v2 &amp; Design Team Handoff Notes
                </h2>
              </div>
              <button
                onClick={() => setShowTauriModal(false)}
                className="p-1 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <div className="space-y-3 text-xs text-slate-300 leading-relaxed">
              <div className="p-3 rounded-xl bg-slate-950 border border-slate-800 space-y-1.5">
                <div className="font-semibold text-cyan-300">
                  1. Frameless Desktop Window (`tauri.conf.json`)
                </div>
                <p className="text-slate-400">
                  The top bar uses <code className="text-amber-300">data-tauri-drag-region</code> so
                  users can drag the native OS window seamlessly. Set{' '}
                  <code className="text-cyan-300">"decorations": false</code> in{' '}
                  <code className="text-cyan-300">tauri.conf.json</code> and wire the top-right
                  window buttons to <code className="text-cyan-300">getCurrentWindow().minimize()</code>,{' '}
                  <code className="text-cyan-300">toggleMaximize()</code>, and{' '}
                  <code className="text-cyan-300">close()</code>.
                </p>
              </div>

              <div className="p-3 rounded-xl bg-slate-950 border border-slate-800 space-y-1.5">
                <div className="font-semibold text-cyan-300">
                  2. Cross-Panel SVG Bezier Tethers
                </div>
                <p className="text-slate-400">
                  The cyan &amp; amber Bezier curves dynamically compute bounding boxes between the
                  active left phrase card, the live kinetic word nodes in the 16:9 stage, and the
                  vertical stem timeline on the right—preserving visual continuity across all 3 columns.
                </p>
              </div>

              <div className="p-3 rounded-xl bg-slate-950 border border-slate-800 space-y-1.5">
                <div className="font-semibold text-cyan-300">
                  3. Consolidated Token &amp; Curve Architecture
                </div>
                <p className="text-slate-400">
                  Replaced the 4 competing rows of micro-pills with a single interactive{' '}
                  <strong>Bracketed Token Bar</strong> and a 3-column{' '}
                  <strong>Advanced Animation Engine</strong> featuring a live cubic-bezier easing graph.
                </p>
              </div>
            </div>

            <div className="flex justify-end pt-2">
              <button
                onClick={() => setShowTauriModal(false)}
                className="px-4 py-1.5 rounded-lg bg-cyan-500 hover:bg-cyan-400 text-slate-950 font-semibold text-xs transition"
              >
                Got it, back to Studio
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}