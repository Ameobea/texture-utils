<script lang="ts">
  export const prerender = true;

  import { maybeInitSentry } from '../sentry';

  maybeInitSentry();
</script>

<slot />

<style>
  :global(.custom-dropzone) {
    background-color: #121212 !important;
    color: #e8e8e8 !important;
    width: max(80%, 300px);
    margin-left: auto;
    margin-right: auto;
  }

  :global(.custom-dropzone button) {
    font-size: 18px;
    background-color: #333;
    outline: none;
    border: 1px solid #888;
    padding: 4px 8px;
  }

  :global(.custom-dropzone p) {
    margin-bottom: 8px;
    margin-top: 16px;
  }
</style>
