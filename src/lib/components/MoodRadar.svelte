<script lang="ts">
  import * as d3 from 'd3';

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
    moodA:   MoodValues;
    moodB?:  MoodValues;
    colorA?: string;
    colorB?: string;
  }

  let { moodA, moodB, colorA = '#00f0ff', colorB = '#ff7c5c' }: Props = $props();

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

  function render() {
    if (!svgEl) return;
    d3.select(svgEl).selectAll('*').remove();

    const N  = AXES.length;
    const W  = svgEl.clientWidth  || 220;
    const H  = svgEl.clientHeight || 200;
    const cx = W / 2, cy = H / 2;
    const R  = Math.min(cx, cy) - 28;
    const g  = d3.select(svgEl).append('g');

    [0.25, 0.5, 0.75, 1.0].forEach(r => {
      g.append('circle')
        .attr('cx', cx).attr('cy', cy).attr('r', R * r)
        .attr('fill', 'none')
        .attr('stroke', 'rgba(255,255,255,0.08)')
        .attr('stroke-width', 1);
    });

    AXES.forEach((ax, i) => {
      const angle = (i / N) * 2 * Math.PI - Math.PI / 2;
      g.append('line')
        .attr('x1', cx).attr('y1', cy)
        .attr('x2', cx + R * Math.cos(angle))
        .attr('y2', cy + R * Math.sin(angle))
        .attr('stroke', 'rgba(255,255,255,0.12)')
        .attr('stroke-width', 1);
      g.append('text')
        .attr('x', cx + (R + 14) * Math.cos(angle))
        .attr('y', cy + (R + 14) * Math.sin(angle))
        .attr('text-anchor', 'middle')
        .attr('dominant-baseline', 'middle')
        .style('font-family', 'JetBrains Mono, monospace')
        .style('font-size', '8px')
        .style('fill', '#849495')
        .text(ax.label);
    });

    const drawPolygon = (mood: MoodValues, color: string) => {
      const pts = AXES.map((ax, i) => {
        const val   = mood[ax.key] ?? 0;
        const angle = (i / N) * 2 * Math.PI - Math.PI / 2;
        return `${cx + R * val * Math.cos(angle)},${cy + R * val * Math.sin(angle)}`;
      });
      g.append('polygon')
        .attr('points', pts.join(' '))
        .attr('fill', color)
        .attr('fill-opacity', 0.15)
        .attr('stroke', color)
        .attr('stroke-width', 1.5);
    };

    drawPolygon(moodA, colorA);
    if (moodB) drawPolygon(moodB, colorB);
  }

  $effect(() => {
    void moodA; void moodB; void colorA; void colorB;
    requestAnimationFrame(() => requestAnimationFrame(render));
  });
</script>

<svg bind:this={svgEl} class="mood-radar"></svg>

<style>
  .mood-radar {
    display: block;
    width: 100%;
    height: 100%;
  }
</style>
