import { invoke } from '$lib/ipc';
import { ui } from '$lib/stores/ui.svelte';
import type { Playlist, SavedSearch, PlaylistTrack } from '$lib/types';

/**
 * @concept TagCuration
 * Manages playlists, playlist track organization, and saved search queries.
 */
function createCurationStore() {
  let playlists = $state<Playlist[]>([]);
  let savedSearches = $state<SavedSearch[]>([]);
  let activePlaylist = $state<Playlist | null>(null);
  let activePlaylistTracks = $state<PlaylistTrack[]>([]);
  let activeSavedSearch = $state<SavedSearch | null>(null);

  async function loadAll() {
    try {
      playlists = await invoke('get_playlists');
      savedSearches = await invoke('get_saved_searches');
    } catch (e: any) {
      console.error('Failed to load curations:', e);
      ui.showToast('Failed to load playlists or saved searches: ' + e, 'error');
    }
  }

  async function createPlaylist(name: string) {
    try {
      const id = await invoke('create_playlist', { name });
      await loadAll();
      ui.showToast('Created playlist "' + name + '"', 'success');
      return id;
    } catch (e: any) {
      console.error('Failed to create playlist:', e);
      ui.showToast(e, 'error');
      return null;
    }
  }

  async function deletePlaylist(id: number) {
    try {
      await invoke('delete_playlist', { id });
      if (activePlaylist?.id === id) {
        activePlaylist = null;
        activePlaylistTracks = [];
        ui.activeView = 'table';
      }
      await loadAll();
      ui.showToast('Deleted playlist', 'success');
    } catch (e: any) {
      console.error('Failed to delete playlist:', e);
      ui.showToast(e, 'error');
    }
  }

  async function renamePlaylist(id: number, newName: string) {
    try {
      await invoke('rename_playlist', { id, newName });
      if (activePlaylist?.id === id) {
        activePlaylist.name = newName;
      }
      await loadAll();
      ui.showToast('Renamed playlist to "' + newName + '"', 'success');
    } catch (e: any) {
      console.error('Failed to rename playlist:', e);
      ui.showToast(e, 'error');
    }
  }

  async function loadPlaylistTracks(playlistId: number) {
    try {
      activePlaylistTracks = await invoke('get_playlist_tracks', { playlistId });
    } catch (e: any) {
      console.error('Failed to load playlist tracks:', e);
      ui.showToast('Failed to load playlist tracks: ' + e, 'error');
    }
  }

  async function addTracksToPlaylist(playlistId: number, trackIds: number[]) {
    try {
      await invoke('add_tracks_to_playlist', { playlistId, trackIds });
      const pl = playlists.find(p => p.id === playlistId);
      const name = pl ? pl.name : 'playlist';
      ui.showToast(`Added ${trackIds.length} track(s) to playlist "${name}"`, 'success');
      if (activePlaylist?.id === playlistId) {
        await loadPlaylistTracks(playlistId);
      }
    } catch (e: any) {
      console.error('Failed to add tracks to playlist:', e);
      ui.showToast(e, 'error');
    }
  }

  async function removeTrackFromPlaylist(playlistId: number, position: number) {
    try {
      await invoke('remove_track_from_playlist', { playlistId, position });
      if (activePlaylist?.id === playlistId) {
        await loadPlaylistTracks(playlistId);
      }
      ui.showToast('Removed track from playlist', 'success');
    } catch (e: any) {
      console.error('Failed to remove track from playlist:', e);
      ui.showToast(e, 'error');
    }
  }

  async function getPlaylistsForTrack(trackId: number) {
    try {
      return await invoke('get_playlists_for_track', { trackId });
    } catch (e: any) {
      console.error('Failed to get playlists for track:', e);
      return [];
    }
  }

  async function removeTrackFromPlaylistById(playlistId: number, trackId: number) {
    try {
      await invoke('remove_track_from_playlist_by_id', { playlistId, trackId });
      if (activePlaylist?.id === playlistId) {
        await loadPlaylistTracks(playlistId);
      }
      ui.showToast('Removed track from playlist', 'success');
    } catch (e: any) {
      console.error('Failed to remove track from playlist by ID:', e);
      ui.showToast(e, 'error');
    }
  }

  async function reorderPlaylistTrack(playlistId: number, fromPos: number, toPos: number) {
    try {
      await invoke('reorder_playlist_track', { playlistId, fromPos, toPos });
      if (activePlaylist?.id === playlistId) {
        await loadPlaylistTracks(playlistId);
      }
    } catch (e: any) {
      console.error('Failed to reorder playlist track:', e);
      ui.showToast(e, 'error');
    }
  }

  async function createSavedSearch(name: string, queryJson: string) {
    try {
      const id = await invoke('create_saved_search', { name, queryJson });
      await loadAll();
      ui.showToast('Saved search "' + name + '" created successfully', 'success');
      ui.sidebarTab = 'curations';
      return id;
    } catch (e: any) {
      console.error('Failed to save search:', e);
      ui.showToast(e, 'error');
      return null;
    }
  }

  async function deleteSavedSearch(id: number) {
    try {
      await invoke('delete_saved_search', { id });
      if (activeSavedSearch?.id === id) {
        activeSavedSearch = null;
      }
      await loadAll();
      ui.showToast('Deleted saved search', 'success');
    } catch (e: any) {
      console.error('Failed to delete saved search:', e);
      ui.showToast(e, 'error');
    }
  }

  async function updateSavedSearch(id: number, queryJson: string) {
    try {
      await invoke('update_saved_search', { id, queryJson });
      await loadAll();
      ui.showToast('Saved search query updated successfully', 'success');
    } catch (e: any) {
      console.error('Failed to update saved search:', e);
      ui.showToast(e, 'error');
    }
  }

  return {
    get playlists() { return playlists; },
    get savedSearches() { return savedSearches; },
    get activePlaylist() { return activePlaylist; },
    set activePlaylist(v) { activePlaylist = v; },
    get activePlaylistTracks() { return activePlaylistTracks; },
    set activePlaylistTracks(v) { activePlaylistTracks = v; },
    get activeSavedSearch() { return activeSavedSearch; },
    set activeSavedSearch(v) { activeSavedSearch = v; },
    loadAll,
    createPlaylist,
    deletePlaylist,
    renamePlaylist,
    loadPlaylistTracks,
    addTracksToPlaylist,
    removeTrackFromPlaylist,
    removeTrackFromPlaylistById,
    getPlaylistsForTrack,
    reorderPlaylistTrack,
    createSavedSearch,
    deleteSavedSearch,
    updateSavedSearch
  };
}

export const curation = createCurationStore();
