// The Animation canvas library. Konva (MIT, npm `konva`) draws the 2D EffectCraft frame and the
// joint handles; Three.js (MIT, npm `three`) renders the 3D Tripo / Blender GLB + skeleton +
// animation clips in the same stage.
import Konva from 'konva';
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';

const win = window as unknown as Record<string, unknown>;
win.Konva = Konva;
win.THREE = {
  ...THREE,
  OrbitControls,
  GLTFLoader,
};
