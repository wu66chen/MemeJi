export interface DetailPanelState {
  panelVisible: boolean;
  hasSelectedMeme: boolean;
  canShowDetailsFromMenu: boolean;
}

/** Keep the detail column mounted so selection changes never resize the gallery. */
export function getDetailPanelState(
  detailsOpen: boolean,
  selectedCount: number,
): DetailPanelState {
  const hasSelectedMeme = selectedCount === 1;
  return {
    panelVisible: detailsOpen,
    hasSelectedMeme,
    canShowDetailsFromMenu: shouldShowDetailsFromMenu(detailsOpen, selectedCount),
  };
}

export function shouldShowDetailsFromMenu(detailsOpen: boolean, selectedCount: number): boolean {
  return !detailsOpen && selectedCount === 1;
}
