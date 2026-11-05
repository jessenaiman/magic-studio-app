import React from 'react';
import { createRoot } from 'react-dom/client';
import Lyrics from '../../references/magic_studio_kinetic_lyric_editor.tsx';

export function mount(container) {
  if (container.shadowRoot) return;
  const shadow = container.attachShadow({ mode: 'open' });
  const css = document.createElement('link');
  css.rel = 'stylesheet';
  css.href = 'lyrics-built/app.css';
  const root = document.createElement('div');
  shadow.append(css, root);
  createRoot(root).render(<Lyrics />);
}

const standalone = document.getElementById('lyrics');
if (standalone) mount(standalone);
