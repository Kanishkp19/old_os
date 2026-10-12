<script>
  import { native, command, media, json, bytes } from '../api.js';
  export let error;
  export let cursor;
  export let playlists;
  export let playlist;
  export let music;
  export let playing;
  export let queuedTracks;
  export let shuffle;
  export let repeat;
  export let form;
  export let formValue;
  export let t;
  export let musicTracks;
  export let run;
  export let load;
  export let more;
  export let confirm;
  export let addTrack;
  export let nextTrack;
  let player, volume = 1;
</script>

      {#if !native}<p>{t('Private playlists are available in the installed Windows app.')}</p>{/if}
      <div class="toolbar"><button class:active={!playlist} on:click={() => playlist = null}>{t('All music')}</button>{#each playlists as row}<button class:active={playlist?.id === row.id} on:click={() => playlist = row}>{row.title}</button>{/each}{#if native}<button on:click={() => { form = { action: 'playlist' }; formValue = ''; }}>{t('New playlist')}</button>{/if}<label class="check"><input type="checkbox" bind:checked={shuffle}/>{t('Shuffle')}</label><label class="check"><input type="checkbox" bind:checked={repeat}/>{t('Repeat')}</label></div>
      {#if playing}<section class="card player"><strong>{playing.name}</strong><audio bind:this={player} src={media(playing.id)} controls autoplay loop={repeat} on:ended={() => nextTrack(1,true)} on:error={() => { error = t("This track could not be played."); playing = null; }}></audio><label>{t('Volume')}<input type="range" min="0" max="1" step="0.01" bind:value={volume} on:input={() => { if (player) player.volume = Number(volume); }}/></label><button on:click={() => nextTrack(-1)}>{t('Previous')}</button><button on:click={() => nextTrack()}>{t('Next')}</button></section>{/if}
      <section class="card"><h2>{t("Up next")}</h2>{#each queuedTracks as queued, index}<div class="row"><span>{queued.name}</span><button on:click={() => queuedTracks = queuedTracks.filter((_, i) => i !== index)}>{t("Remove")}</button></div>{:else}<p>{t("Queue is empty")}</p>{/each}</section><section class="card">{#each musicTracks as track}<div class="row"><button class="link" on:click={() => playing = track}>{t('Play')} · {track.name}</button><span>{bytes(track.size)}</span><button on:click={() => queuedTracks = [...queuedTracks, track]}>{t("Add to queue")}</button>{#if playlist}<button on:click={() => run(() => addTrack(track,true))}>{t('Remove')}</button>{/if}</div>{:else}<p>{t('Nothing here yet.')}</p>{/each}</section>{#if playlist}<section class="card"><h2>{t('Add to playlist')}</h2>{#each music.filter(f => !json(playlist.body,{ids:[]}).ids?.includes(f.id)) as track}<button on:click={() => run(() => addTrack(track))}>{track.name}</button>{/each}<button class="danger" on:click={() => confirm(t('Remove'), async () => { await command('local_trash',{id:playlist.id}); playlist = null; await load(); })}>{t('Remove')} {playlist.title}</button></section>{/if}{#if cursor}<button on:click={() => run(more)}>{t('Load more')}</button>{/if}
