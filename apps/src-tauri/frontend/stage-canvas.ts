// The Animation canvas library. Konva (MIT, npm `konva`) draws the EffectCraft frame and the
// joint handles; it is exposed as `window.Konva` for the classic UI scripts.
import Konva from 'konva';

(window as unknown as { Konva: typeof Konva }).Konva = Konva;
