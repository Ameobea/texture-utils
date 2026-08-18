<script lang="ts">
  import SvelteSeo from 'svelte-seo';
</script>

<SvelteSeo
  title="Ameo Texture Utils"
  description="A collection of web-based tools for generating + manipulating textures for WebGL and other applications."
  openGraph={{
    title: 'Ameo Texture Utils',
    description:
      'A collection of web-based tools for generating + manipulating textures for WebGL and other applications.',
    type: 'website',
  }}
/>

<!-- Github corner -->
<!-- svelte-ignore security-anchor-rel-noreferrer -->
<a
  href="https://github.com/ameobea/texture-utils"
  class="github-corner"
  aria-label="View source on GitHub"
  target="_blank"
>
  <svg
    width="80"
    height="80"
    viewBox="0 0 250 250"
    style="fill:#64CEAA; color:#fff; position: absolute; top: 0; border: 0; right: 0;"
    aria-hidden="true"
  >
    <path d="M0,0 L115,115 L130,115 L142,142 L250,250 L250,0 Z" /><path
      d="M128.3,109.0 C113.8,99.7 119.0,89.6 119.0,89.6 C122.0,82.7 120.5,78.6 120.5,78.6 C119.2,72.0 123.4,76.3 123.4,76.3 C127.3,80.9 125.5,87.3 125.5,87.3 C122.9,97.6 130.6,101.9 134.4,103.2"
      fill="currentColor"
      style="transform-origin: 130px 106px;"
      class="octo-arm"
    />
    <path
      d="M115.0,115.0 C114.9,115.1 118.7,116.5 119.8,115.4 L133.7,101.6 C136.9,99.2 139.9,98.4 142.2,98.6 C133.8,88.0 127.5,74.4 143.8,58.0 C148.5,53.4 154.0,51.2 159.7,51.0 C160.3,49.4 163.2,43.6 171.4,40.1 C171.4,40.1 176.1,42.5 178.8,56.2 C183.1,58.6 187.2,61.8 190.9,65.4 C194.5,69.0 197.7,73.2 200.1,77.6 C213.8,80.2 216.3,84.9 216.3,84.9 C212.7,93.1 206.9,96.0 205.4,96.6 C205.1,102.4 203.0,107.8 198.3,112.5 C181.9,128.9 168.3,122.5 157.7,114.1 C157.9,116.9 156.7,120.9 152.7,124.9 L141.0,136.5 C139.8,137.7 141.6,141.9 141.8,141.8 Z"
      fill="currentColor"
      class="octo-body"
    />
  </svg>
</a>

<div class="root">
  <h1>Ameo Texture Utils</h1>

  <p style="text-align: center;">
    A collection of web-based tools for generating + manipulating textures for WebGL and other
    applications.
  </p>

  <div class="utils">
    <div class="util">
      <a href="/seamless-stitcher"><h2>Seamless Texture Stitcher</h2></a>
      <p>
        Takes multiple similar seamless textures and stitches them together into a bigger output
        texture, retaining their seamlessness.
      </p>
      <p>
        Works great for taking several similar seamless textures generated via stable diffusion or
        other tools and creating 2k or higher textures from them.
      </p>
    </div>
    <div class="util">
      <a href="/seamless-tile-maker"><h2>Seamless Tile Maker</h2></a>
      <p>
        Takes a single near-seamless texture and produces a same-size output that tiles cleanly by
        blending the image with a half-period-shifted copy of itself in a configurable margin
        around the seams.
      </p>
      <p>
        Optional variance-preserving (contrast-corrected) blending preserves detail in the seam
        region for textures with strong high-frequency content.
      </p>
    </div>
    <div class="util">
      <a href="/single-channel-compressor"><h2>Single-Channel Texture Compressor</h2></a>
      <p>
        Uses pallette reduction to pack an RGB texture into a single 256-color channel. Allows for
        the other channels to be used for things like a normal map.
      </p>
      <p>
        Note that the outputs from this will not be technically correct for a lot of things like
        interpolation, mipmap generation, and other things. However, I've found that it's good
        enough for the things I use it for.
      </p>
    </div>
    <div class="util">
      <a href="/color-ramp-builder"><h2>Color Ramp Builder</h2></a>
      <p>Tool to create a color ramp that can be used to map a grayscale texture to color.</p>
    </div>
    <div class="util">
      <a href="/noise-signature"><h2>Noise Signature Extractor</h2></a>
      <p>
        Fits a compact spectral fingerprint (~32 bytes) + color ramp to a texture photo and
        re-synthesizes a seamless texture from just those parameters.
      </p>
      <p>
        Outputs copy-pastable Geoscript code. Works best on stochastic materials (concrete, sand,
        stucco, wood grain); structured layouts like bricks won't survive — by design.
      </p>
    </div>
    <div class="util">
      <a href="/spectral-noise-playground"><h2>Spectral Noise Playground</h2></a>
      <p>
        Interactive playground for the <code>spectral_noise</code> synthesizer: hand-edit the
        log-polar band spectrum, spectral peaks, and color ramp with live previews of both the
        texture and its model spectrum.
      </p>
      <p>
        A learning tool for building an intuition of how spectra map to texture appearance.
        Emits the same copy-pastable Geoscript snippets as the extractor.
      </p>
    </div>
    <div class="util">
      <a href="/normal-map-composer"><h2>Normal Map Composer</h2></a>
      <p>
        Blends two tangent-space normal maps using
        <a href="https://colinbarrebrisebois.com/category/reoriented-normal-mapping/">
          Reoriented Normal Mapping</a
        >.
      </p>
    </div>
    <div class="util">
      <a href="/normal-map-filter"><h2>Normal Map Filter</h2></a>
      <p>
        Filter normal maps to extract or remove detail at specific frequency scales.
        Supports low-pass, high-pass, band-pass, and band-reject modes using Gaussian filtering.
      </p>
    </div>
    <div class="util">
      <a href="/reverse-color-ramp"><h2>Reverse Color Ramp</h2></a>
      <p>
        Generates a GLSL function that maps diffuse color to a scalar value (for use in things like
        generating roughness/metalness) based on a user-defined reverse color ramp.
      </p>
    </div>
    <div class="util">
      <a href="/distortion-correction"><h2>Distortion Correction</h2></a>
      <p>
        Corrects perspective (keystone) distortion in photos of flat surfaces — tiled floors,
        textured walls — to produce clean, square diffuse maps for 3D rendering.
      </p>
      <p>
        Auto-detects the distortion from straight lines in the image (Canny + Hough vanishing
        points), with manual corner and guide-line controls for fine-tuning.
      </p>
    </div>
    <div class="util">
      <a href="/svg-path-editor"><h2>SVG Path Editor</h2></a>
      <p>
        Edit simple open SVG paths on a grid with snapping, point insertion/removal, and live path
        output.
      </p>
    </div>
  </div>

  <footer>
    <!-- svelte-ignore security-anchor-rel-noreferrer -->
    Created by <a target="_blank" href="https://cprimozic.net">Casey Primozic / ameo</a>
  </footer>
</div>

<style lang="css">
  .root {
    display: flex;
    flex-direction: column;
    min-height: 100vh;
    justify-content: center;
    font-size: 18px;
  }

  h1,
  h2 {
    text-align: center;
  }

  .utils {
    display: flex;
    flex-direction: row;
    justify-content: center;
    margin-left: auto;
    margin-right: auto;
    flex-wrap: wrap;
    max-width: 1400px;
  }

  .utils .util {
    flex: 1;
    margin: 1rem;
    padding: 1rem;
    border: 1px solid #ccc;
    border-radius: 0.5rem;
    min-width: 400px;
    max-width: 800px;
  }

  footer {
    margin-top: 2rem;
    margin-bottom: 4px;
    text-align: center;
  }

  .github-corner:hover .octo-arm {
    animation: octocat-wave 560ms ease-in-out;
  }
  @keyframes octocat-wave {
    0%,
    100% {
      transform: rotate(0);
    }
    20%,
    60% {
      transform: rotate(-25deg);
    }
    40%,
    80% {
      transform: rotate(10deg);
    }
  }
  @media (max-width: 500px) {
    .github-corner:hover .octo-arm {
      animation: none;
    }
    .github-corner .octo-arm {
      animation: octocat-wave 560ms ease-in-out;
    }
  }
</style>
