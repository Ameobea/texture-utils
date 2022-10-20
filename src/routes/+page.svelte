<script lang="ts">
</script>

<div class="root">
  <h1>Ameo Texture Utils</h1>

  <!-- TODO github badge/link -->

  <div class="utils">
    <div class="util">
      <a href="/seamless-stitcher"><h2>Seamless Texture Stitcher</h2></a>
      <p>
        Takes multiple similar seamless textures and stitches them together into a bigger output
        texture.
      </p>
    </div>
    <div class="util">
      <a href="/single-channel-compressor"><h2>Single-Channel Texture Compressor</h2></a>
      <p>
        Uses pallette reduction to pack an RGB texture into a single 256-color channel, reducing GPU
        memory and allowing the other channels to be used for things like a normal map.
      </p>
    </div>
  </div>
</div>

<style lang="css">
  .root {
    display: flex;
    flex-direction: column;
    height: 100vh;
    justify-content: center;
  }

  h1,
  h2 {
    text-align: center;
  }

  .utils {
    display: flex;
    flex-direction: row;
    margin-left: 10vw;
    margin-right: 10vw;
  }

  .utils .util {
    flex: 1;
    margin: 1rem;
    padding: 1rem;
    border: 1px solid #ccc;
    border-radius: 0.5rem;
  }
</style>
