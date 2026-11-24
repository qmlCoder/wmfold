<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';
import * as THREE from 'three';
import { TrackballControls } from 'three/examples/jsm/controls/TrackballControls.js';
import { api } from '../api/tauri';
import type { MolStructure, RemoteProfile } from '../types';

const props = defineProps<{ path: string; profile?: RemoteProfile | null }>();

const host = ref<HTMLDivElement | null>(null);
const loading = ref(false);
const message = ref('');
const atomCount = ref(0);
const bondCount = ref(0);

const MIN_HEIGHT = 150;
const MAX_HEIGHT = 640;
const DEFAULT_HEIGHT = 280;
const HEIGHT_KEY = 'wmfold.moleculeHeight';

function loadHeight(): number {
  const saved = Number(localStorage.getItem(HEIGHT_KEY));
  if (Number.isFinite(saved) && saved >= MIN_HEIGHT && saved <= MAX_HEIGHT) {
    return saved;
  }
  return DEFAULT_HEIGHT;
}

const stageHeight = ref(loadHeight());
let resizeStartY = 0;
let resizeStartHeight = 0;
let resizing = false;

function maxHeightForWindow(): number {
  return Math.max(MIN_HEIGHT, Math.min(MAX_HEIGHT, window.innerHeight - 180));
}

function onResizeMove(event: PointerEvent) {
  if (!resizing) return;
  const delta = event.clientY - resizeStartY;
  stageHeight.value = Math.max(
    MIN_HEIGHT,
    Math.min(maxHeightForWindow(), resizeStartHeight + delta),
  );
}

function stopResize() {
  if (!resizing) return;
  resizing = false;
  window.removeEventListener('pointermove', onResizeMove);
  window.removeEventListener('pointerup', stopResize);
  document.body.style.userSelect = '';
  localStorage.setItem(HEIGHT_KEY, String(Math.round(stageHeight.value)));
}

function startResize(event: PointerEvent) {
  resizing = true;
  resizeStartY = event.clientY;
  resizeStartHeight = stageHeight.value;
  document.body.style.userSelect = 'none';
  window.addEventListener('pointermove', onResizeMove);
  window.addEventListener('pointerup', stopResize);
}

let renderer: THREE.WebGLRenderer | null = null;
let scene: THREE.Scene | null = null;
let camera: THREE.PerspectiveCamera | null = null;
let controls: TrackballControls | null = null;
let resizeObserver: ResizeObserver | null = null;
let molecule: THREE.Group | null = null;
let raf = 0;
let disposed = false;
let requestId = 0;
let fitRadius = 5;

// 常见元素颜色（接近 Jmol/CPK，但降低饱和度以适配浅色界面）
const ELEMENT_COLORS: Record<string, number> = {
  H: 0xffffff,
  D: 0xdddddd,
  C: 0x3f4650,
  N: 0x3f6fd8,
  O: 0xd94a3d,
  F: 0x5fbf7f,
  P: 0xe08a2e,
  S: 0xd6b52c,
  Cl: 0x3fae5a,
  Br: 0xb5533c,
  I: 0x8a5bb8,
  B: 0xd98a6a,
  Si: 0xb6a07a,
  Li: 0xa66fb8,
  Na: 0x8f6fd8,
  K: 0x7a6fd8,
  Mg: 0x6fa86f,
  Ca: 0x8fa86f,
  Fe: 0xc77b3e,
  Zn: 0x8a8f98,
  Cu: 0xb87333,
  Ni: 0x7f9f7f,
  Mn: 0x9b7fb8,
  He: 0x9fd8e8,
  Ne: 0x8fd8e8,
  Ar: 0x8fd0e8,
};

// 显示半径（Å），在真实共价半径基础上做球棍模型的视觉缩放
const ELEMENT_RADII: Record<string, number> = {
  H: 0.25,
  D: 0.26,
  C: 0.38,
  N: 0.37,
  O: 0.36,
  F: 0.34,
  P: 0.44,
  S: 0.44,
  Cl: 0.42,
  Br: 0.46,
  I: 0.5,
  B: 0.4,
  Si: 0.45,
  Fe: 0.46,
  Zn: 0.46,
  Cu: 0.46,
};

function colorFor(symbol: string): number {
  return ELEMENT_COLORS[symbol] ?? 0x8a8f98;
}

function radiusFor(symbol: string): number {
  return ELEMENT_RADII[symbol] ?? 0.4;
}

function disposeMolecule() {
  if (!molecule) return;
  molecule.traverse((obj) => {
    const mesh = obj as THREE.Mesh;
    if (!mesh.isMesh) return;
    mesh.geometry?.dispose();
    const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
    for (const material of materials) material?.dispose();
  });
  scene?.remove(molecule);
  molecule = null;
}

function initScene() {
  const el = host.value;
  if (!el) return;
  try {
    scene = new THREE.Scene();
    camera = new THREE.PerspectiveCamera(42, 1, 0.05, 5000);
    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    renderer.setSize(Math.max(el.clientWidth, 1), Math.max(el.clientHeight, 1));
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    renderer.setClearColor(0x000000, 0);
    el.appendChild(renderer.domElement);

    controls = new TrackballControls(camera, renderer.domElement);
    controls.rotateSpeed = 2.8;
    controls.zoomSpeed = 1.2;
    controls.panSpeed = 0.8;
    controls.staticMoving = false;
    controls.dynamicDampingFactor = 0.14;
    controls.minDistance = 0.8;
    controls.maxDistance = 800;

    scene.add(new THREE.HemisphereLight(0xffffff, 0x596273, 2.4));
    const key = new THREE.DirectionalLight(0xffffff, 2.7);
    key.position.set(6, 9, 8);
    scene.add(key);
    const fill = new THREE.DirectionalLight(0xbcd4ff, 0.9);
    fill.position.set(-7, -4, -6);
    scene.add(fill);

    resizeObserver = new ResizeObserver(() => {
      // 面板宽度/高度变化时，等浏览器完成布局后再同步渲染尺寸，
      // 同时让 TrackballControls 重新计算屏幕坐标，否则鼠标位置会错位。
      requestAnimationFrame(() => resize());
    });
    resizeObserver.observe(el);
    resize();

    const loop = () => {
      if (disposed) return;
      raf = requestAnimationFrame(loop);
      controls?.update();
      if (renderer && scene && camera) renderer.render(scene, camera);
    };
    loop();
  } catch (err) {
    message.value = `三维预览初始化失败：${String(err)}`;
  }
}

function resize() {
  const el = host.value;
  if (!el || !renderer || !camera) return;
  const width = Math.max(el.clientWidth, 1);
  const height = Math.max(el.clientHeight, 1);
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.setSize(width, height, true);
  camera.aspect = width / height;
  camera.updateProjectionMatrix();
  controls?.handleResize();
}

function resetView() {
  if (!camera || !controls) return;
  const r = Math.max(fitRadius, 1.2);
  camera.position.set(r * 2.0, r * 1.35, r * 2.15);
  camera.near = 0.05;
  camera.far = Math.max(1000, r * 80);
  camera.updateProjectionMatrix();
  controls.target.set(0, 0, 0);
  controls.update();
}

function halfBond(
  group: THREE.Group,
  start: THREE.Vector3,
  end: THREE.Vector3,
  color: number,
  radius: number,
  geometry: THREE.CylinderGeometry,
) {
  const delta = end.clone().sub(start);
  const length = delta.length();
  if (length < 1e-6) return;
  const material = new THREE.MeshStandardMaterial({
    color,
    roughness: 0.45,
    metalness: 0.05,
  });
  const mesh = new THREE.Mesh(geometry, material);
  mesh.position.copy(start).add(end).multiplyScalar(0.5);
  mesh.quaternion.setFromUnitVectors(
    new THREE.Vector3(0, 1, 0),
    delta.clone().normalize(),
  );
  mesh.scale.set(radius, length, radius);
  group.add(mesh);
}

function buildMolecule(structure: MolStructure) {
  disposeMolecule();
  if (!scene) return;
  const atoms = structure.atoms;
  if (!atoms.length) return;

  const positions: THREE.Vector3[] = [];
  const center = new THREE.Vector3();
  for (const atom of atoms) {
    const p = new THREE.Vector3(atom.x, atom.y, atom.z);
    positions.push(p);
    center.add(p);
  }
  center.multiplyScalar(1 / atoms.length);
  for (const p of positions) p.sub(center);

  const group = new THREE.Group();
  const sphereGeometry = new THREE.SphereGeometry(1, 24, 18);
  const bondGeometry = new THREE.CylinderGeometry(1, 1, 1, 12);
  const bondRadius = 0.085;
  let maxDistance = 0;

  atoms.forEach((atom, index) => {
    const radius = radiusFor(atom.element);
    const material = new THREE.MeshStandardMaterial({
      color: colorFor(atom.element),
      roughness: 0.32,
      metalness: 0.05,
    });
    const sphere = new THREE.Mesh(sphereGeometry, material);
    sphere.position.copy(positions[index]);
    sphere.scale.setScalar(radius);
    group.add(sphere);
    maxDistance = Math.max(maxDistance, positions[index].length() + radius);
  });

  for (const [i, j] of structure.bonds) {
    if (i >= positions.length || j >= positions.length) continue;
    const start = positions[i];
    const end = positions[j];
    const mid = start.clone().add(end).multiplyScalar(0.5);
    halfBond(group, start, mid, colorFor(atoms[i].element), bondRadius, bondGeometry);
    halfBond(group, mid, end, colorFor(atoms[j].element), bondRadius, bondGeometry);
  }

  fitRadius = Math.max(maxDistance, 1.2);
  scene.add(group);
  molecule = group;
  controls?.target.set(0, 0, 0);
  resetView();
}

async function loadStructure() {
  const id = ++requestId;
  disposeMolecule();
  atomCount.value = 0;
  bondCount.value = 0;
  message.value = '';

  if (!props.path) {
    message.value = '没有可预览的文件';
    return;
  }

  loading.value = true;
  try {
    const structure = props.profile
      ? await api.remoteStructure(props.profile, props.path)
      : await api.readStructure(props.path);
    if (id !== requestId) return;
    if (!structure.ok || !structure.atoms.length) {
      message.value = '该文件没有可显示的三维坐标';
      return;
    }
    atomCount.value = structure.atoms.length;
    bondCount.value = structure.bonds.length;
    buildMolecule(structure);
  } catch (err) {
    if (id === requestId) message.value = `读取三维结构失败：${String(err)}`;
  } finally {
    if (id === requestId) loading.value = false;
  }
}

onMounted(() => {
  initScene();
  loadStructure();
});

watch(
  () =>
    [
      props.path,
      props.profile?.id ?? '',
      props.profile?.host ?? '',
      props.profile?.port ?? 0,
    ] as const,
  () => loadStructure(),
);

onBeforeUnmount(() => {
  stopResize();
  disposed = true;
  requestId++;
  cancelAnimationFrame(raf);
  resizeObserver?.disconnect();
  disposeMolecule();
  controls?.dispose();
  renderer?.dispose();
  if (renderer?.domElement.parentElement) {
    renderer.domElement.parentElement.removeChild(renderer.domElement);
  }
  renderer = null;
  scene = null;
  camera = null;
  controls = null;
  resizeObserver = null;
});
</script>

<template>
  <div class="wm-mol-viewer">
    <div class="wm-mol-toolbar">
      <span class="wm-mol-title">三维结构</span>
      <span v-if="atomCount" class="wm-mol-count">
        {{ atomCount }} 原子 · {{ bondCount }} 键
      </span>
      <button
        v-if="atomCount && !message"
        class="wm-mol-reset"
        type="button"
        title="重置视角"
        @click="resetView"
      >
        重置
      </button>
    </div>
    <div
      ref="host"
      class="wm-mol-stage"
      :style="{ height: stageHeight + 'px' }"
    >
      <div v-if="loading" class="wm-mol-overlay">正在解析结构…</div>
      <div v-else-if="message" class="wm-mol-overlay">{{ message }}</div>
    </div>
    <div
      class="wm-mol-resizer"
      title="拖动调整三维预览高度"
      @pointerdown.prevent="startResize"
    ></div>
    <div v-if="atomCount && !message" class="wm-mol-hint">
      左键旋转 · 滚轮缩放 · 右键平移
    </div>
  </div>
</template>
