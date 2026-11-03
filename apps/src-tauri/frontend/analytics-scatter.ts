import {BufferGeometry, Color, Float32BufferAttribute, OrthographicCamera, Points, PointsMaterial, Scene, WebGLRenderer} from 'three';

type Row = {label: string; ccn: number; nloc: number};
function mount(node: HTMLElement, rows: Row[]) {
  const renderer = new WebGLRenderer({antialias: true, alpha: true, powerPreference: 'low-power'});
  renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
  renderer.domElement.setAttribute('role', 'img');
  renderer.domElement.setAttribute('aria-label', 'Saved function hotspots: horizontal axis NLOC, vertical axis CCN. Exact values in evidence below.');
  const scene = new Scene();
  const camera = new OrthographicCamera(-0.1, 1.1, 1.1, -0.1, 0.1, 10);
  camera.position.z = 2;
  const maxX = Math.max(1, ...rows.map(row => row.nloc));
  const maxY = Math.max(1, ...rows.map(row => row.ccn));
  const positions: number[] = [], colors: number[] = [];
  const style = getComputedStyle(node);
  const normal = new Color(style.getPropertyValue('--accent').trim());
  const warning = new Color(style.getPropertyValue('--amber').trim());
  for (const row of rows) {
    positions.push(row.nloc / maxX, row.ccn / maxY, 0);
    const color = row.ccn > 15 ? warning : normal;
    colors.push(color.r, color.g, color.b);
  }
  const geometry = new BufferGeometry();
  geometry.setAttribute('position', new Float32BufferAttribute(positions, 3));
  geometry.setAttribute('color', new Float32BufferAttribute(colors, 3));
  const material = new PointsMaterial({size: 9, sizeAttenuation: false, vertexColors: true});
  scene.add(new Points(geometry, material));
  const caption = document.createElement('p');
  caption.className = 'mayhem-chart-note';
  caption.textContent = `NLOC → 0–${maxX} · CCN ↑ 0–${maxY} · amber: CCN > 15 · ${rows.length} saved hotspots (not every function)`;
  const lost = (event: Event) => { event.preventDefault(); caption.textContent = 'WebGL context lost. Disable and enable the view to retry; exact evidence remains below.'; };
  renderer.domElement.addEventListener('webglcontextlost', lost);
  node.append(renderer.domElement, caption);
  const draw = () => {
    if (!node.isConnected || !node.clientWidth) return;
    renderer.setSize(node.clientWidth, Math.max(220, node.clientHeight - caption.offsetHeight), false);
    renderer.render(scene, camera);
  };
  const observer = new ResizeObserver(draw);
  observer.observe(node);
  try { draw(); } catch (error) {
    observer.disconnect(); geometry.dispose(); material.dispose(); renderer.dispose(); renderer.forceContextLoss(); node.replaceChildren();
    throw error;
  }
  return {dispose() {
    observer.disconnect();
    renderer.domElement.removeEventListener('webglcontextlost', lost);
    geometry.dispose(); material.dispose(); scene.clear(); renderer.dispose(); renderer.forceContextLoss();
    node.replaceChildren();
  }};
}

Object.assign(globalThis, {magicMayhemScatter: {mount}});
