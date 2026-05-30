export type ActiveView = 'table' | 'map' | 'analysis' | 'settings';

function createUiStore() {
  let activeView = $state<ActiveView>('table');
  let mapFocusTrackId = $state<number | null>(null);
  let errorMessage = $state('');
  let successMessage = $state('');
  let toastTimeout: ReturnType<typeof setTimeout> | undefined;

  function showToast(msg: string, type: 'success' | 'error') {
    clearTimeout(toastTimeout);
    if (type === 'error') {
      errorMessage = msg;
      successMessage = '';
    } else {
      successMessage = msg;
      errorMessage = '';
    }
    toastTimeout = setTimeout(() => {
      errorMessage = '';
      successMessage = '';
    }, 4500);
  }

  function focusMapTrack(trackId: number) {
    mapFocusTrackId = trackId;
    activeView = 'map';
  }

  return {
    get activeView() { return activeView; },
    set activeView(v: ActiveView) { activeView = v; },
    get mapFocusTrackId() { return mapFocusTrackId; },
    set mapFocusTrackId(v: number | null) { mapFocusTrackId = v; },
    get errorMessage() { return errorMessage; },
    get successMessage() { return successMessage; },
    showToast,
    focusMapTrack,
  };
}

export const ui = createUiStore();
