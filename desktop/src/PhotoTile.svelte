<script>
  import { media } from './api.js';
  export let photo;
  export let label;
  export let t = key => key;
  let failed = false;
</script>

{#if photo.thumb_status === 'ready' && !failed}
  <img src={media(photo.file_id, 'thumb')} alt={photo.name || label} loading="lazy" on:error={() => failed = true}/>
{:else}
  <span class="photo-placeholder" role="img" aria-label={photo.name || label}>
    {photo.thumb_status === 'pending' ? t('Preparing preview') : t('Original preserved; preview unavailable')}
  </span>
{/if}
<span>{label}</span>
