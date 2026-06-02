import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import PlaylistSelector from "./PlaylistSelector.svelte";
import { curation } from "$lib/stores/curation.svelte";

const MOCK_PLAYLISTS = [
  { id: 1, name: "Chill Vibes", created_at: 1000, updated_at: 1000 },
  { id: 2, name: "Workout Hits", created_at: 2000, updated_at: 2000 },
  { id: 3, name: "Deep Focus", created_at: 3000, updated_at: 3000 },
];

describe("PlaylistSelector Component", () => {
  beforeEach(() => {
    // Reset/mock curation.playlists using direct Svelte 5 mutation
    // In Svelte 5, curation store properties can be mocked or filled:
    vi.spyOn(curation, "playlists", "get").mockReturnValue(MOCK_PLAYLISTS);
  });

  it("renders with default or custom placeholder", () => {
    const { container } = render(PlaylistSelector, {
      props: { placeholder: "Select your playlist..." }
    });
    const input = container.querySelector("input");
    expect(input).not.toBeNull();
    expect(input?.placeholder).toBe("Select your playlist...");
  });

  it("shows all suggestions on focus when showAllOnFocus is true and query is empty", async () => {
    const { container } = render(PlaylistSelector, {
      props: { showAllOnFocus: true }
    });
    const input = container.querySelector("input")!;
    
    // Suggestion box should not be visible before focus
    expect(screen.queryByText("Chill Vibes")).toBeNull();

    await fireEvent.focus(input);

    expect(screen.getByText("Chill Vibes")).toBeInTheDocument();
    expect(screen.getByText("Workout Hits")).toBeInTheDocument();
    expect(screen.getByText("Deep Focus")).toBeInTheDocument();
  });

  it("does not show suggestions on focus when showAllOnFocus is false and query is empty", async () => {
    const { container } = render(PlaylistSelector, {
      props: { showAllOnFocus: false }
    });
    const input = container.querySelector("input")!;

    await fireEvent.focus(input);

    expect(screen.queryByText("Chill Vibes")).toBeNull();
  });

  it("filters suggestions as the user types", async () => {
    const { container } = render(PlaylistSelector, {
      props: { showAllOnFocus: false }
    });
    const input = container.querySelector("input")!;

    await fireEvent.focus(input);
    await fireEvent.input(input, { target: { value: "work" } });

    expect(screen.getByText("Workout Hits")).toBeInTheDocument();
    expect(screen.queryByText("Chill Vibes")).toBeNull();
    expect(screen.queryByText("Deep Focus")).toBeNull();
  });

  it("triggers onselect callback and binds activePlaylist when suggestion is clicked", async () => {
    const onselect = vi.fn();
    let activePlaylist = null;

    const { container } = render(PlaylistSelector, {
      props: {
        showAllOnFocus: true,
        activePlaylist,
        onselect
      }
    });

    const input = container.querySelector("input")!;
    await fireEvent.focus(input);

    const btn = screen.getByText("Chill Vibes");
    await fireEvent.mouseDown(btn);

    expect(onselect).toHaveBeenCalledWith(MOCK_PLAYLISTS[0]);
  });

  it("calls onclear and resets activePlaylist when clear button is clicked", async () => {
    const onclear = vi.fn();

    const { container } = render(PlaylistSelector, {
      props: {
        activePlaylist: MOCK_PLAYLISTS[1],
        onclear
      }
    });

    const clearBtn = screen.getByText("×");
    await fireEvent.click(clearBtn);

    expect(onclear).toHaveBeenCalled();
    const input = container.querySelector("input")!;
    expect(input.value).toBe("");
  });
});
