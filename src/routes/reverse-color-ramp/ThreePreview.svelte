<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import type {
    MeshPhysicalMaterial,
    PerspectiveCamera,
    Scene,
    Texture,
    Vector3,
    WebGLRenderer,
    Mesh,
  } from 'three';
  import type { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
  import type { ReverseColorRampParams } from './glslGenerator';
  import {
    buildReverseColorRampUniformData,
    buildReverseColorRampUniformFunction,
  } from './glslGenerator';

  export let inputImage: HTMLImageElement | null = null;
  export let roughnessParams: ReverseColorRampParams;
  export let metalnessParams: ReverseColorRampParams;
  export let useRoughness = true;
  export let useMetalness = false;
  export let active = true;

  let container: HTMLDivElement | null = null;
  let renderer: WebGLRenderer | null = null;
  let scene: Scene | null = null;
  let camera: PerspectiveCamera | null = null;
  let mesh: Mesh | null = null;
  let material: MeshPhysicalMaterial | null = null;
  let texture: Texture | null = null;
  let controls: OrbitControls | null = null;
  let resizeObserver: ResizeObserver | null = null;
  let rafId: number | null = null;
  let animationActive = false;
  let ready = false;

  const roughnessPrefix = 'roughnessRamp';
  const metalnessPrefix = 'metalnessRamp';
  const roughnessFnName = 'roughnessRampFromBaseColor';
  const metalnessFnName = 'metalnessRampFromBaseColor';

  const applyUniformData = (
    uniforms: Record<string, { value: unknown }>,
    prefix: string,
    data: ReturnType<typeof buildReverseColorRampUniformData>
  ) => {
    (uniforms[`${prefix}A`].value as Vector3).set(...data.A_lin);
    (uniforms[`${prefix}U`].value as Vector3).set(...data.U);
    uniforms[`${prefix}InvLen`].value = data.invLen;
    uniforms[`${prefix}VMin`].value = data.vMin;
    uniforms[`${prefix}VMax`].value = data.vMax;
    uniforms[`${prefix}CurveSteepness`].value = data.curveSteepness;
    uniforms[`${prefix}CurveOffset`].value = data.curveOffset;
    uniforms[`${prefix}PerpSigma`].value = data.perpSigma;
    uniforms[`${prefix}BaseFallback`].value = data.baseFallback;
  };

  const syncUniforms = () => {
    if (!material || !material.userData.reverseColorRampShader) {
      return;
    }

    const shader = material.userData.reverseColorRampShader as any;
    const uniforms = shader.uniforms as Record<string, { value: unknown }>;
    const roughnessData = buildReverseColorRampUniformData(roughnessParams);
    const metalnessData = buildReverseColorRampUniformData(metalnessParams);

    applyUniformData(uniforms, roughnessPrefix, roughnessData);
    applyUniformData(uniforms, metalnessPrefix, metalnessData);
    uniforms.roughnessRampEnabled.value = useRoughness ? 1 : 0;
    uniforms.metalnessRampEnabled.value = useMetalness ? 1 : 0;
  };

  const updateTexture = async () => {
    if (!material || !inputImage || !renderer) {
      return;
    }

    if (!inputImage.complete || inputImage.naturalWidth === 0) {
      const pendingImage = inputImage;
      pendingImage.addEventListener(
        'load',
        () => {
          if (pendingImage === inputImage) {
            void updateTexture();
          }
        },
        { once: true }
      );
      return;
    }

    const THREE = await import('three');
    const nextTexture = new THREE.Texture(inputImage);
    const textureAny = nextTexture as unknown as {
      colorSpace?: string;
      encoding?: number;
    };

    if ('colorSpace' in textureAny) {
      textureAny.colorSpace = THREE.SRGBColorSpace;
    } else {
      textureAny.encoding = THREE.sRGBEncoding;
    }

    nextTexture.needsUpdate = true;
    texture?.dispose();
    texture = nextTexture;
    material.map = texture;
    material.needsUpdate = true;
  };

  const resizeToContainer = () => {
    if (!container || !renderer || !camera) {
      return;
    }

    const { width, height } = container.getBoundingClientRect();
    if (width <= 0 || height <= 0) {
      return;
    }

    renderer.setSize(width, height, false);
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
  };

  const patchMaterial = async (mat: MeshPhysicalMaterial) => {
    const THREE = await import('three');
    const roughnessRampGlsl = buildReverseColorRampUniformFunction(
      roughnessFnName,
      roughnessPrefix
    );
    const metalnessRampGlsl = buildReverseColorRampUniformFunction(
      metalnessFnName,
      metalnessPrefix
    );
    const rampToggleUniforms = `uniform float roughnessRampEnabled;
uniform float metalnessRampEnabled;`;

    mat.onBeforeCompile = (shader: any) => {
      shader.uniforms[`${roughnessPrefix}A`] = { value: new THREE.Vector3() };
      shader.uniforms[`${roughnessPrefix}U`] = { value: new THREE.Vector3(0, 0, 0) };
      shader.uniforms[`${roughnessPrefix}InvLen`] = { value: 1 };
      shader.uniforms[`${roughnessPrefix}VMin`] = { value: 0 };
      shader.uniforms[`${roughnessPrefix}VMax`] = { value: 1 };
      shader.uniforms[`${roughnessPrefix}CurveSteepness`] = { value: 1 };
      shader.uniforms[`${roughnessPrefix}CurveOffset`] = { value: 0.5 };
      shader.uniforms[`${roughnessPrefix}PerpSigma`] = { value: 0 };
      shader.uniforms[`${roughnessPrefix}BaseFallback`] = { value: 0 };

      shader.uniforms[`${metalnessPrefix}A`] = { value: new THREE.Vector3() };
      shader.uniforms[`${metalnessPrefix}U`] = { value: new THREE.Vector3(0, 0, 0) };
      shader.uniforms[`${metalnessPrefix}InvLen`] = { value: 1 };
      shader.uniforms[`${metalnessPrefix}VMin`] = { value: 0 };
      shader.uniforms[`${metalnessPrefix}VMax`] = { value: 1 };
      shader.uniforms[`${metalnessPrefix}CurveSteepness`] = { value: 1 };
      shader.uniforms[`${metalnessPrefix}CurveOffset`] = { value: 0.5 };
      shader.uniforms[`${metalnessPrefix}PerpSigma`] = { value: 0 };
      shader.uniforms[`${metalnessPrefix}BaseFallback`] = { value: 0 };

      shader.uniforms.roughnessRampEnabled = { value: useRoughness ? 1 : 0 };
      shader.uniforms.metalnessRampEnabled = { value: useMetalness ? 1 : 0 };

      shader.fragmentShader = shader.fragmentShader.replace(
        '#include <common>',
        `#include <common>\n${rampToggleUniforms}\n${roughnessRampGlsl}\n${metalnessRampGlsl}`
      );

      shader.fragmentShader = shader.fragmentShader.replace(
        '#include <metalnessmap_fragment>',
        `#include <metalnessmap_fragment>
float roughnessRampValue = ${roughnessFnName}(diffuseColor.rgb);
float metalnessRampValue = ${metalnessFnName}(diffuseColor.rgb);
roughnessFactor = mix(roughnessFactor, roughnessRampValue, roughnessRampEnabled);
metalnessFactor = mix(metalnessFactor, metalnessRampValue, metalnessRampEnabled);`
      );

      mat.userData.reverseColorRampShader = shader;
      syncUniforms();
    };
  };

  const renderFrame = () => {
    if (!renderer || !scene || !camera) {
      return;
    }
    controls?.update();
    renderer.render(scene, camera);
  };

  const animate = () => {
    if (!animationActive) {
      return;
    }
    renderFrame();
    rafId = requestAnimationFrame(animate);
  };

  const startLoop = () => {
    if (animationActive) {
      return;
    }
    animationActive = true;
    rafId = requestAnimationFrame(animate);
  };

  const stopLoop = () => {
    animationActive = false;
    if (rafId !== null) {
      cancelAnimationFrame(rafId);
      rafId = null;
    }
  };

  const init = async () => {
    if (!container) {
      return;
    }

    const THREE = await import('three');
    scene = new THREE.Scene();
    scene.background = new THREE.Color(0x151515);

    const { width, height } = container.getBoundingClientRect();
    camera = new THREE.PerspectiveCamera(35, width / height, 0.1, 30);
    camera.position.set(1.35, 1.0, 1.35);
    camera.lookAt(0, 0, 0);

    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
    const rendererAny = renderer as unknown as {
      outputColorSpace?: string;
      outputEncoding?: number;
    };
    if ('outputColorSpace' in rendererAny) {
      rendererAny.outputColorSpace = THREE.SRGBColorSpace;
    } else {
      // Legacy fallback for older Three.js versions (sRGBEncoding = 3001)
      rendererAny.outputEncoding = 3001;
    }
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.setSize(width, height, false);
    container.appendChild(renderer.domElement);

    const keyLight = new THREE.DirectionalLight(0xffffff, 1.25);
    keyLight.position.set(2.2, 3.2, 1.4);
    scene.add(keyLight);

    const fillLight = new THREE.DirectionalLight(0xffffff, 0.5);
    fillLight.position.set(-2.2, 1.4, 1.8);
    scene.add(fillLight);

    const rimLight = new THREE.DirectionalLight(0xffffff, 0.35);
    rimLight.position.set(-1.6, 1.8, -2.4);
    scene.add(rimLight);

    const ambient = new THREE.AmbientLight(0xffffff, 0.15);
    scene.add(ambient);

    const geometry = new THREE.BoxGeometry(1.4, 1.4, 1.4);
    material = new THREE.MeshPhysicalMaterial({
      color: 0xffffff,
      roughness: 0.6,
      metalness: 0.1,
    });
    await patchMaterial(material);

    mesh = new THREE.Mesh(geometry, material);
    scene.add(mesh);

    const controlsModule = await import('three/examples/jsm/controls/OrbitControls.js');
    controls = new controlsModule.OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.08;
    controls.minDistance = 0.85;
    controls.maxDistance = 3.0;
    controls.target.set(0, 0, 0);
    controls.update();

    await updateTexture();
    syncUniforms();

    if (active) {
      startLoop();
    }

    resizeObserver = new ResizeObserver(() => resizeToContainer());
    resizeObserver.observe(container);
    resizeToContainer();
    ready = true;
  };

  onMount(() => {
    void init();
  });

  onDestroy(() => {
    stopLoop();
    resizeObserver?.disconnect();
    controls?.dispose();
    texture?.dispose();
    material?.dispose();
    mesh?.geometry.dispose();
    renderer?.dispose();
    renderer?.domElement.remove();
  });

  $: if (ready && inputImage) {
    void updateTexture();
  }

  $: if (ready) {
    roughnessParams;
    metalnessParams;
    useRoughness;
    useMetalness;
    syncUniforms();
  }

  $: if (ready) {
    if (controls) {
      controls.enabled = active;
    }
    if (active) {
      resizeToContainer();
      renderFrame();
      startLoop();
    } else {
      stopLoop();
    }
  }
</script>

<div class="preview-root" bind:this={container} />

<style>
  .preview-root {
    width: 100%;
    height: 100%;
    background: radial-gradient(circle at 20% 20%, #2a2a2a, #0f0f0f 65%, #090909);
    border: 1px solid #333;
    border-radius: 8px;
    overflow: hidden;
  }
</style>
