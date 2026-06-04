<script lang="ts">
  import * as d3 from 'd3';
  import { theme } from '$lib/stores/theme.svelte';

  export interface MoodValues {
    happy:      number | null;
    sad:        number | null;
    aggressive: number | null;
    relaxed:    number | null;
    party:      number | null;
    acoustic:   number | null;
    electronic: number | null;
  }

  interface Props {
    moodA:          MoodValues;
    moodB?:         MoodValues;
    colorA?:        string;
    colorB?:        string;
    interactive?:   boolean;
    thresholds?:    Partial<MoodValues>;
    onAxisClick?:   (key: keyof MoodValues, value: number | null) => void;
    onClear?:       () => void;
  }

  let { moodA, moodB, colorA, colorB, interactive = false, thresholds = {}, onAxisClick, onClear }: Props = $props();

  const AXES: { key: keyof MoodValues; label: string }[] = [
    { key: 'happy',      label: 'Happy'      },
    { key: 'party',      label: 'Party'      },
    { key: 'electronic', label: 'Electronic' },
    { key: 'aggressive', label: 'Aggressive' },
    { key: 'sad',        label: 'Sad'        },
    { key: 'relaxed',    label: 'Relaxed'    },
    { key: 'acoustic',   label: 'Acoustic'   },
  ];

  let svgEl: SVGSVGElement;
  let ghostAxis: number | null = null;
  let ghostValue: number = 0;

  const TOGGLE_RADIUS = 12;

  function getGeometry() {
    const W  = svgEl.clientWidth  || 220;
    const H  = svgEl.clientHeight || 200;
    const cx = W / 2, cy = H / 2;
    const R  = Math.min(cx, cy) - 28;
    return { W, H, cx, cy, R };
  }

  function axisAngle(i: number) {
    return (i / AXES.length) * 2 * Math.PI - Math.PI / 2;
  }

  function getAxisValueFromPoint(x: number, y: number, cx: number, cy: number, R: number): { axisIdx: number; value: number } {
    let best = -1, bestDist = Infinity;
    AXES.forEach((_, i) => {
      const angle = axisAngle(i);
      // project (x,y) onto axis direction
      const dx = Math.cos(angle), dy = Math.sin(angle);
      const dot = (x - cx) * dx + (y - cy) * dy;
      // perpendicular distance from axis line
      const px = (x - cx) - dot * dx;
      const py = (y - cy) - dot * dy;
      const perp = Math.sqrt(px * px + py * py);
      if (perp < bestDist) { bestDist = perp; best = i; }
    });
    const angle = axisAngle(best);
    const dot = (x - cx) * Math.cos(angle) + (y - cy) * Math.sin(angle);
    const value = Math.max(0, Math.min(1, dot / R));
    return { axisIdx: best, value };
  }

  function handleMouseMove(e: MouseEvent) {
    if (!interactive) return;
    const rect = svgEl.getBoundingClientRect();
    const { cx, cy, R } = getGeometry();
    const { axisIdx, value } = getAxisValueFromPoint(e.clientX - rect.left, e.clientY - rect.top, cx, cy, R);
    ghostAxis = axisIdx;
    ghostValue = value;
    render();
  }

  function handleMouseLeave() {
    ghostAxis = null;
    render();
  }

  function handleClick(e: MouseEvent) {
    if (!interactive || !onAxisClick) return;
    const rect = svgEl.getBoundingClientRect();
    const { cx, cy, R } = getGeometry();
    const { axisIdx, value } = getAxisValueFromPoint(e.clientX - rect.left, e.clientY - rect.top, cx, cy, R);
    const key = AXES[axisIdx].key;
    const existing = thresholds[key];
    if (existing != null) {
      // check if click is near the existing vertex — toggle off
      const angle = axisAngle(axisIdx);
      const vx = cx + R * existing * Math.cos(angle);
      const vy = cy + R * existing * Math.sin(angle);
      const mx = e.clientX - rect.left, my = e.clientY - rect.top;
      const dist = Math.sqrt((mx - vx) ** 2 + (my - vy) ** 2);
      if (dist <= TOGGLE_RADIUS) { onAxisClick(key, null); return; }
    }
    onAxisClick(key, Math.round(value * 100) / 100);
  }

  function render() {
    if (!svgEl) return;
    d3.select(svgEl).selectAll('*').remove();

    const isLight = theme.resolvedTheme === 'light';

    const resolvedColorA  = colorA ?? (isLight ? '#0284c7' : '#00f0ff');
    const resolvedColorB  = colorB ?? (isLight ? '#dc4e2a' : '#ff7c5c');
    const threshColor     = isLight ? '#7c3aed' : '#bc13fe';
    const gridStroke      = isLight ? 'rgba(0,0,0,0.10)'  : 'rgba(255,255,255,0.08)';
    const axisStroke      = isLight ? 'rgba(0,0,0,0.14)'  : 'rgba(255,255,255,0.12)';
    const axisActiveStroke= isLight ? 'rgba(0,0,0,0.28)'  : 'rgba(255,255,255,0.28)';
    const labelFill       = isLight ? '#475569'            : '#849495';
    const labelActiveFill = isLight ? '#1e293b'            : '#e3e1e9';

    const N  = AXES.length;
    const { cx, cy, R } = getGeometry();
    const g  = d3.select(svgEl).append('g');

    // grid rings
    [0.25, 0.5, 0.75, 1.0].forEach(r => {
      g.append('circle')
        .attr('cx', cx).attr('cy', cy).attr('r', R * r)
        .attr('fill', 'none')
        .attr('stroke', gridStroke)
        .attr('stroke-width', 1);
    });

    // axes + labels
    AXES.forEach((ax, i) => {
      const angle   = axisAngle(i);
      const isActive = interactive && thresholds[ax.key] != null;
      const isGhost  = interactive && ghostAxis === i;
      g.append('line')
        .attr('x1', cx).attr('y1', cy)
        .attr('x2', cx + R * Math.cos(angle))
        .attr('y2', cy + R * Math.sin(angle))
        .attr('stroke', isActive ? axisActiveStroke : axisStroke)
        .attr('stroke-width', 1);
      g.append('text')
        .attr('x', cx + (R + 14) * Math.cos(angle))
        .attr('y', cy + (R + 14) * Math.sin(angle))
        .attr('text-anchor', 'middle')
        .attr('dominant-baseline', 'middle')
        .style('font-family', 'JetBrains Mono, monospace')
        .style('font-size', '8px')
        .style('font-weight', isActive || isGhost ? '700' : '400')
        .style('fill', isActive || isGhost ? labelActiveFill : labelFill)
        .style('cursor', interactive ? 'pointer' : 'default')
        .text(ax.label);
    });

    // threshold polygon + vertices
    if (interactive) {
      const activeAxes = AXES.filter(ax => thresholds[ax.key] != null);
      if (activeAxes.length > 0) {
        const pts = AXES.map((ax, i) => {
          const val   = thresholds[ax.key] ?? 0;
          const angle = axisAngle(i);
          return `${cx + R * val * Math.cos(angle)},${cy + R * val * Math.sin(angle)}`;
        });
        g.append('polygon')
          .attr('points', pts.join(' '))
          .attr('fill', threshColor)
          .attr('fill-opacity', 0.12)
          .attr('stroke', threshColor)
          .attr('stroke-width', 1.5)
          .attr('stroke-dasharray', '3,2');
      }

      // active vertices
      AXES.forEach((ax, i) => {
        const val = thresholds[ax.key];
        if (val == null) return;
        const angle = axisAngle(i);
        g.append('circle')
          .attr('cx', cx + R * val * Math.cos(angle))
          .attr('cy', cy + R * val * Math.sin(angle))
          .attr('r', 4)
          .attr('fill', threshColor)
          .attr('stroke', isLight ? '#fff' : '#161b22')
          .attr('stroke-width', 1.5)
          .style('cursor', 'pointer');
        // value label
        g.append('text')
          .attr('x', cx + (R * val + 12) * Math.cos(angle))
          .attr('y', cy + (R * val + 12) * Math.sin(angle))
          .attr('text-anchor', 'middle')
          .attr('dominant-baseline', 'middle')
          .style('font-family', 'JetBrains Mono, monospace')
          .style('font-size', '7px')
          .style('fill', threshColor)
          .text(`${Math.round(val * 100)}%`);
      });

      // ghost vertex
      if (ghostAxis !== null) {
        const angle = axisAngle(ghostAxis);
        g.append('circle')
          .attr('cx', cx + R * ghostValue * Math.cos(angle))
          .attr('cy', cy + R * ghostValue * Math.sin(angle))
          .attr('r', 3)
          .attr('fill', threshColor)
          .attr('fill-opacity', 0.4)
          .attr('stroke', threshColor)
          .attr('stroke-width', 1)
          .style('pointer-events', 'none');
      }
    }

    // data polygons
    const drawPolygon = (mood: MoodValues, color: string) => {
      const pts = AXES.map((ax, i) => {
        const val   = mood[ax.key] ?? 0;
        const angle = axisAngle(i);
        return `${cx + R * val * Math.cos(angle)},${cy + R * val * Math.sin(angle)}`;
      });
      g.append('polygon')
        .attr('points', pts.join(' '))
        .attr('fill', color)
        .attr('fill-opacity', isLight ? 0.25 : 0.15)
        .attr('stroke', color)
        .attr('stroke-width', isLight ? 2 : 1.5);
    };

    drawPolygon(moodA, resolvedColorA);
    if (moodB) drawPolygon(moodB, resolvedColorB);
  }

  $effect(() => {
    void moodA; void moodB; void colorA; void colorB;
    void theme.resolvedTheme;
    void thresholds; void ghostAxis; void ghostValue;
    requestAnimationFrame(() => requestAnimationFrame(render));
  });
</script>

<svg
  bind:this={svgEl}
  class="mood-radar"
  class:interactive
  role={interactive ? 'slider' : undefined}
  aria-label={interactive ? 'Mood filter radar' : undefined}
  onmousemove={interactive ? handleMouseMove : undefined}
  onmouseleave={interactive ? handleMouseLeave : undefined}
  onclick={interactive ? handleClick : undefined}
  ondblclick={interactive ? () => onClear?.() : undefined}
  onkeydown={interactive ? (e) => { if (e.key === 'Escape') { AXES.forEach(ax => onAxisClick?.(ax.key, null)); } } : undefined}
></svg>

<style>
  .mood-radar {
    display: block;
    width: 100%;
    height: 100%;
  }

  .mood-radar.interactive {
    cursor: crosshair;
  }
</style>
