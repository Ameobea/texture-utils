<script lang="ts">
  import { onMount } from 'svelte';
  import SvelteSeo from 'svelte-seo';

  type Point = { x: number; y: number };
  type SegmentType = 'line' | 'quadratic';
  type Segment = { type: SegmentType; control?: Point };

  const viewBoxSize = 100;
  const minPoints = 2;

  let gridStep = 5;
  let snapStep = 1;
  let snapEnabled = true;
  let showGrid = true;

  let points: Point[] = [
    { x: -35, y: -20 },
    { x: -10, y: -30 },
    { x: 20, y: -5 },
    { x: 35, y: 25 },
  ];
  let segments: Segment[] = points.slice(1).map(() => ({ type: 'line' }));

  let svgEl: SVGSVGElement | null = null;
  let dragTarget: { kind: 'point' | 'control'; index: number } | null = null;
  let lastPointerId: number | null = null;
  let isPanning = false;
  let panAnchor: { x: number; y: number; centerX: number; centerY: number } | null = null;
  let viewCenterX = 0;
  let viewCenterY = 0;
  let zoom = 1;
  const handleScaleExponent = 0.6;
  const basePointRadius = 0.7;
  let svgSize = { width: 1, height: 1 };

  $: safeGridStep = Math.max(1, Number.isFinite(gridStep) ? gridStep : 1);
  $: safeSnapStep = Math.max(0.1, Number.isFinite(snapStep) ? snapStep : 0.1);
  $: svgAspect = svgSize.width > 0 && svgSize.height > 0 ? svgSize.width / svgSize.height : 1;
  $: viewBoxWidth = (viewBoxSize / zoom) * (svgAspect >= 1 ? svgAspect : 1);
  $: viewBoxHeight = (viewBoxSize / zoom) * (svgAspect >= 1 ? 1 : 1 / svgAspect);
  $: viewBoxX = viewCenterX - viewBoxWidth / 2;
  $: viewBoxY = viewCenterY - viewBoxHeight / 2;
  $: viewBoxString = `${viewBoxX} ${viewBoxY} ${viewBoxWidth} ${viewBoxHeight}`;
  $: pointRadius = clamp(basePointRadius / Math.pow(zoom, handleScaleExponent), 0.45, 2.2);
  $: controlRadius = clamp(
    (basePointRadius * 0.75) / Math.pow(zoom, handleScaleExponent),
    0.35,
    1.8
  );

  const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

  const getBaseViewBoxDims = () => {
    if (svgAspect >= 1) {
      return { width: viewBoxSize * svgAspect, height: viewBoxSize };
    }
    return { width: viewBoxSize, height: viewBoxSize / svgAspect };
  };

  const snapValue = (value: number, step: number) => Math.round(value / step) * step;

  const formatNumber = (value: number) => {
    const rounded = Math.round(value * 1000) / 1000;
    return Number.isInteger(rounded) ? `${rounded}` : rounded.toString();
  };

  const toPathString = (pts: Point[], segs: Segment[]) => {
    if (pts.length === 0) {
      return '';
    }

    let d = `M ${formatNumber(pts[0].x)} ${formatNumber(pts[0].y)}`;
    for (let i = 0; i < segs.length; i++) {
      const seg = segs[i];
      const end = pts[i + 1];
      if (!end) {
        continue;
      }
      if (seg.type === 'quadratic' && seg.control) {
        d += ` Q ${formatNumber(seg.control.x)} ${formatNumber(seg.control.y)} ${formatNumber(
          end.x
        )} ${formatNumber(end.y)}`;
      } else {
        d += ` L ${formatNumber(end.x)} ${formatNumber(end.y)}`;
      }
    }
    return d;
  };

  $: pathD = toPathString(points, segments);

  const updatePoint = (index: number, next: Partial<Point>) => {
    points = points.map((point, i) => (i === index ? { ...point, ...next } : point));
  };

  const updateControl = (index: number, next: Partial<Point>) => {
    segments = segments.map((seg, i) =>
      i === index && seg.control ? { ...seg, control: { ...seg.control, ...next } } : seg
    );
  };

  const setSegmentType = (index: number, type: SegmentType) => {
    segments = segments.map((seg, i) => {
      if (i !== index) {
        return seg;
      }

      if (type === 'line') {
        return { type };
      }

      const start = points[index];
      const end = points[index + 1];
      const control = seg.control
        ? seg.control
        : {
            x: (start.x + end.x) / 2,
            y: (start.y + end.y) / 2,
          };

      return { type, control };
    });
  };

  const clientToSvgPoint = (event: PointerEvent | MouseEvent): Point => {
    if (!svgEl) {
      return { x: 0, y: 0 };
    }

    const rect = svgEl.getBoundingClientRect();
    const x = viewBoxX + ((event.clientX - rect.left) / rect.width) * viewBoxWidth;
    const y = viewBoxY + ((event.clientY - rect.top) / rect.height) * viewBoxHeight;
    return { x, y };
  };

  const updateSvgSize = () => {
    if (!svgEl) {
      return;
    }
    const rect = svgEl.getBoundingClientRect();
    if (rect.width && rect.height) {
      svgSize = { width: rect.width, height: rect.height };
    }
  };

  const applySnapping = (point: Point, event?: PointerEvent | MouseEvent) => {
    if (!snapEnabled || (event && event.altKey)) {
      return point;
    }

    return {
      x: snapValue(point.x, safeSnapStep),
      y: snapValue(point.y, safeSnapStep),
    };
  };

  const getPathBounds = () => {
    const allPoints: Point[] = [...points];
    segments.forEach(segment => {
      if (segment.type === 'quadratic' && segment.control) {
        allPoints.push(segment.control);
      }
    });

    if (!allPoints.length) {
      return null;
    }

    let minX = allPoints[0].x;
    let maxX = allPoints[0].x;
    let minY = allPoints[0].y;
    let maxY = allPoints[0].y;

    for (const pt of allPoints) {
      minX = Math.min(minX, pt.x);
      maxX = Math.max(maxX, pt.x);
      minY = Math.min(minY, pt.y);
      maxY = Math.max(maxY, pt.y);
    }

    return { minX, maxX, minY, maxY };
  };

  const centerViewOnPath = () => {
    const bounds = getPathBounds();
    if (!bounds) {
      return;
    }

    const width = Math.max(bounds.maxX - bounds.minX, 1);
    const height = Math.max(bounds.maxY - bounds.minY, 1);
    const paddedWidth = width * 1.2;
    const paddedHeight = height * 1.2;
    const baseDims = getBaseViewBoxDims();
    const zoomX = baseDims.width / paddedWidth;
    const zoomY = baseDims.height / paddedHeight;
    const nextZoom = clamp(Math.min(zoomX, zoomY), 0.05, 50);
    const nextWidth = baseDims.width / nextZoom;
    const nextHeight = baseDims.height / nextZoom;
    const centerX = (bounds.minX + bounds.maxX) / 2;
    const centerY = (bounds.minY + bounds.maxY) / 2;

    zoom = nextZoom;
    viewCenterX = centerX;
    viewCenterY = centerY;
  };

  const removePoint = (index: number) => {
    if (points.length <= minPoints) {
      return;
    }

    if (index === 0) {
      points = points.slice(1);
      segments = segments.slice(1);
      return;
    }

    if (index === points.length - 1) {
      points = points.slice(0, -1);
      segments = segments.slice(0, -1);
      return;
    }

    points = points.filter((_, i) => i !== index);
    segments = segments.filter((_, i) => i !== index);
    segments = segments.map((seg, i) => (i === index - 1 ? { type: 'line' } : seg));
  };

  const onPointPointerDown = (event: PointerEvent, index: number) => {
    event.preventDefault();
    event.stopPropagation();
    dragTarget = { kind: 'point', index };
    lastPointerId = event.pointerId;
    svgEl?.setPointerCapture(event.pointerId);
  };

  const onControlPointerDown = (event: PointerEvent, index: number) => {
    event.preventDefault();
    event.stopPropagation();
    dragTarget = { kind: 'control', index };
    lastPointerId = event.pointerId;
    svgEl?.setPointerCapture(event.pointerId);
  };

  const onPointerMove = (event: PointerEvent) => {
    if (isPanning && panAnchor && svgEl) {
      const rect = svgEl.getBoundingClientRect();
      if (!rect.width || !rect.height) {
        return;
      }
      const dx = ((event.clientX - panAnchor.x) / rect.width) * viewBoxWidth;
      const dy = ((event.clientY - panAnchor.y) / rect.height) * viewBoxHeight;
      viewCenterX = panAnchor.centerX - dx;
      viewCenterY = panAnchor.centerY - dy;
      return;
    }

    if (!dragTarget) {
      return;
    }

    const nextPoint = applySnapping(clientToSvgPoint(event), event);
    if (dragTarget.kind === 'point') {
      updatePoint(dragTarget.index, nextPoint);
    } else {
      updateControl(dragTarget.index, nextPoint);
    }
  };

  const onPointerUp = (event: PointerEvent) => {
    if (dragTarget) {
      dragTarget = null;
    }
    if (isPanning) {
      isPanning = false;
      panAnchor = null;
    }
    if (lastPointerId === event.pointerId) {
      svgEl?.releasePointerCapture(event.pointerId);
      lastPointerId = null;
    }
  };

  const onBackgroundPointerDown = (event: PointerEvent) => {
    if (event.button !== 0 || !svgEl) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    isPanning = true;
    panAnchor = {
      x: event.clientX,
      y: event.clientY,
      centerX: viewCenterX,
      centerY: viewCenterY,
    };
    lastPointerId = event.pointerId;
    svgEl.setPointerCapture(event.pointerId);
  };

  const handleWheel = (event: WheelEvent) => {
    if (!svgEl) {
      return;
    }

    event.preventDefault();
    const delta = -event.deltaY * 0.0015;
    const zoomFactor = Math.exp(delta);
    const nextZoom = clamp(zoom * zoomFactor, 0.05, 50);
    if (nextZoom === zoom) {
      return;
    }

    const cursor = clientToSvgPoint(event);
    const baseDims = getBaseViewBoxDims();
    const nextWidth = baseDims.width / nextZoom;
    const nextHeight = baseDims.height / nextZoom;
    const currentX = viewCenterX - viewBoxWidth / 2;
    const currentY = viewCenterY - viewBoxHeight / 2;
    const fx = viewBoxWidth ? (cursor.x - currentX) / viewBoxWidth : 0.5;
    const fy = viewBoxHeight ? (cursor.y - currentY) / viewBoxHeight : 0.5;
    const nextX = cursor.x - fx * nextWidth;
    const nextY = cursor.y - fy * nextHeight;
    zoom = nextZoom;
    viewCenterX = nextX + nextWidth / 2;
    viewCenterY = nextY + nextHeight / 2;
  };

  const handleContextMenu = (event: MouseEvent, index: number) => {
    event.preventDefault();
    removePoint(index);
  };

  const addPointAtStart = () => {
    const first = points[0];
    const second = points[1];
    const offset = 10;
    const next = first
      ? second
        ? { x: first.x + (first.x - second.x), y: first.y + (first.y - second.y) }
        : { x: first.x - offset, y: first.y - offset }
      : { x: -offset, y: -offset };
    points = [next, ...points];
    segments = [{ type: 'line' }, ...segments];
  };

  const addPointAtEnd = () => {
    const last = points[points.length - 1];
    const prev = points[points.length - 2];
    const offset = 10;
    const next = last
      ? prev
        ? { x: last.x + (last.x - prev.x), y: last.y + (last.y - prev.y) }
        : { x: last.x + offset, y: last.y + offset }
      : { x: offset, y: offset };
    points = [...points, next];
    segments = [...segments, { type: 'line' }];
  };

  const copyPath = async () => {
    if (!navigator.clipboard) {
      return;
    }

    try {
      await navigator.clipboard.writeText(pathD);
    } catch (err) {
      console.error('Failed to copy path', err);
    }
  };

  onMount(() => {
    updateSvgSize();
    const observer = new ResizeObserver(() => updateSvgSize());
    if (svgEl) {
      observer.observe(svgEl);
    }

    const handleKeydown = (event: KeyboardEvent) => {
      if (event.key !== '.') {
        return;
      }

      const target = event.target as HTMLElement | null;
      if (target) {
        const tag = target.tagName?.toLowerCase();
        const isEditable =
          tag === 'input' || tag === 'textarea' || tag === 'select' || target.isContentEditable;
        if (isEditable) {
          return;
        }
      }

      event.preventDefault();
      centerViewOnPath();
    };

    window.addEventListener('keydown', handleKeydown);
    return () => {
      window.removeEventListener('keydown', handleKeydown);
      observer.disconnect();
    };
  });
</script>

<SvelteSeo
  title="SVG Path Editor"
  description="Simple SVG path editor with grid snapping and live path output."
  openGraph={{
    title: 'SVG Path Editor',
    description: 'Simple SVG path editor with grid snapping and live path output.',
    type: 'website',
  }}
/>

<div class="page">
  <header>
    <h1>SVG Path Editor</h1>
    <p>
      Build a single open path by placing points on the grid. Use the buttons to add points, right-
      click a point to remove it, drag the background to pan, and scroll to zoom. Hold Alt while
      dragging to bypass snapping.
    </p>
  </header>

  <div class="layout">
    <section class="canvas-panel">
      <div class="canvas-toolbar">
        <label>
          <input type="checkbox" bind:checked={snapEnabled} />
          Snap to grid
        </label>
        {#if snapEnabled}
          <span class="pill">Snap</span>
        {/if}
        <label>
          Snap step
          <input type="number" min="0.1" step="0.1" bind:value={snapStep} />
        </label>
        <label>
          Grid step
          <input type="number" min="1" step="1" bind:value={gridStep} />
        </label>
        <label>
          <input type="checkbox" bind:checked={showGrid} />
          Show grid
        </label>
        <button type="button" class="ghost" on:click={centerViewOnPath}>Reset view</button>
      </div>

      <div class="svg-wrap">
        <svg
          bind:this={svgEl}
          class:is-panning={isPanning}
          viewBox={viewBoxString}
          on:pointermove={onPointerMove}
          on:pointerup={onPointerUp}
          on:pointerleave={onPointerUp}
          on:wheel|preventDefault={handleWheel}
        >
          <defs>
            <pattern
              id="minor-grid"
              width={safeGridStep}
              height={safeGridStep}
              patternUnits="userSpaceOnUse"
            >
              <path
                d={`M ${safeGridStep} 0 L 0 0 0 ${safeGridStep}`}
                fill="none"
                stroke="#202020"
                stroke-width="0.2"
              />
            </pattern>
            <pattern
              id="major-grid"
              width={safeGridStep * 5}
              height={safeGridStep * 5}
              patternUnits="userSpaceOnUse"
            >
              <rect width={safeGridStep * 5} height={safeGridStep * 5} fill="url(#minor-grid)" />
              <path
                d={`M ${safeGridStep * 5} 0 L 0 0 0 ${safeGridStep * 5}`}
                fill="none"
                stroke="#323232"
                stroke-width="0.3"
              />
            </pattern>
          </defs>

          {#if showGrid}
            <rect
              class="grid-backdrop"
              x={viewBoxX}
              y={viewBoxY}
              width={viewBoxWidth}
              height={viewBoxHeight}
              fill="url(#major-grid)"
              on:pointerdown={onBackgroundPointerDown}
            />
          {:else}
            <rect
              class="grid-backdrop"
              x={viewBoxX}
              y={viewBoxY}
              width={viewBoxWidth}
              height={viewBoxHeight}
              fill="#0c0c0c"
              on:pointerdown={onBackgroundPointerDown}
            />
          {/if}

          <line class="axis-line" x1={viewBoxX} y1={0} x2={viewBoxX + viewBoxWidth} y2={0} />
          <line class="axis-line" x1={0} y1={viewBoxY} x2={0} y2={viewBoxY + viewBoxHeight} />

          <path class="path-hit" d={pathD} />
          <path class="path-visible" d={pathD} />

          {#each segments as segment, i}
            {#if segment.type === 'quadratic' && segment.control}
              <line
                class="control-line"
                x1={points[i].x}
                y1={points[i].y}
                x2={segment.control.x}
                y2={segment.control.y}
              />
              <line
                class="control-line"
                x1={segment.control.x}
                y1={segment.control.y}
                x2={points[i + 1].x}
                y2={points[i + 1].y}
              />
              <circle
                class="control-point"
                cx={segment.control.x}
                cy={segment.control.y}
                r={controlRadius}
                on:pointerdown={event => onControlPointerDown(event, i)}
              />
            {/if}
          {/each}

          {#each points as point, i}
            <circle
              class={i === 0 || i === points.length - 1 ? 'point endpoint' : 'point'}
              cx={point.x}
              cy={point.y}
              r={pointRadius}
              on:pointerdown={event => onPointPointerDown(event, i)}
              on:contextmenu={event => handleContextMenu(event, i)}
            />
          {/each}
        </svg>
      </div>
    </section>

    <aside class="controls">
      <div class="panel">
        <h2>Segments</h2>
        <div class="segment-actions">
          <button type="button" on:click={addPointAtStart}>Add point at start</button>
        </div>
        {#each segments as segment, i}
          <div class="segment-row">
            <div class="segment-header">
              <span>Segment {i + 1}</span>
              <select
                value={segment.type}
                on:change={event => setSegmentType(i, event.currentTarget.value)}
              >
                <option value="line">Line</option>
                <option value="quadratic">Quadratic</option>
              </select>
            </div>
            {#if segment.type === 'quadratic' && segment.control}
              <div class="input-row">
                <label>
                  CX
                  <input
                    type="number"
                    value={segment.control.x}
                    step="0.1"
                    on:input={event => updateControl(i, { x: Number(event.currentTarget.value) })}
                  />
                </label>
                <label>
                  CY
                  <input
                    type="number"
                    value={segment.control.y}
                    step="0.1"
                    on:input={event => updateControl(i, { y: Number(event.currentTarget.value) })}
                  />
                </label>
              </div>
            {/if}
          </div>
        {/each}
        <div class="segment-actions end">
          <button type="button" on:click={addPointAtEnd}>Add point at end</button>
        </div>
      </div>

      <div class="panel">
        <h2>Points</h2>
        {#each points as point, i}
          <div class="point-row">
            <span>P{i + 1}</span>
            <div class="point-inputs">
              <label>
                X
                <input
                  type="number"
                  value={point.x}
                  step="0.1"
                  on:input={event => updatePoint(i, { x: Number(event.currentTarget.value) })}
                />
              </label>
              <label>
                Y
                <input
                  type="number"
                  value={point.y}
                  step="0.1"
                  on:input={event => updatePoint(i, { y: Number(event.currentTarget.value) })}
                />
              </label>
            </div>
            <button
              type="button"
              class="remove"
              disabled={points.length <= minPoints}
              on:click={() => removePoint(i)}
            >
              Remove
            </button>
          </div>
        {/each}
      </div>

      <div class="panel">
        <h2>SVG Path</h2>
        <textarea readonly rows="4" value={pathD} />
        <button type="button" on:click={copyPath}>Copy to clipboard</button>
      </div>
    </aside>
  </div>
</div>

<style lang="css">
  .page {
    padding: 12px 16px;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    gap: 16px;
    height: 100dvh;
    box-sizing: border-box;
    background: #0b0b0b;
    color: #f2f2f2;
    color-scheme: dark;
    overflow: hidden;
  }

  header {
    max-width: 1000px;
    margin: 0 auto;
    text-align: center;
  }

  header h1 {
    margin: 0;
    font-size: 24px;
    letter-spacing: 0.3px;
  }

  header p {
    margin: 6px auto 0;
    max-width: 900px;
    font-size: 13px;
    color: #c8c8c8;
  }

  .layout {
    display: grid;
    grid-template-columns: minmax(0, 2fr) minmax(280px, 1fr);
    gap: 12px;
    align-items: start;
    min-height: 0;
    height: 100%;
  }

  .canvas-panel {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
    height: 100%;
  }

  .canvas-toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    align-items: center;
    background: #121212;
    padding: 8px 10px;
    border: 1px solid #2a2a2a;
    border-radius: 0;
  }

  .canvas-toolbar label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 14px;
  }

  .canvas-toolbar input[type='number'] {
    width: 70px;
    padding: 3px 6px;
    background: #0f0f0f;
    color: #f2f2f2;
    border: 1px solid #2a2a2a;
    border-radius: 0;
  }

  .canvas-toolbar input[type='checkbox'] {
    accent-color: #f2f2f2;
  }

  .canvas-toolbar button {
    padding: 5px 10px;
    border: 1px solid #2a2a2a;
    background: #0f0f0f;
    color: #f2f2f2;
    border-radius: 0;
    cursor: pointer;
  }

  .canvas-toolbar button.ghost {
    border-color: #3a3a3a;
    color: #cfcfcf;
  }

  .canvas-toolbar button:hover,
  .panel button:hover {
    border-color: #3a3a3a;
  }

  .pill {
    padding: 2px 6px;
    border: 1px solid #3a3a3a;
    color: #bfbfbf;
    font-size: 11px;
    letter-spacing: 0.3px;
    text-transform: uppercase;
  }

  .svg-wrap {
    width: 100%;
    background: #0b0b0b;
    border: 1px solid #1f1f1f;
    border-radius: 0;
    overflow: hidden;
    flex: 1;
    min-height: 0;
    height: 100%;
  }

  svg {
    width: 100%;
    height: 100%;
    touch-action: none;
    cursor: default;
  }

  svg.is-panning {
    cursor: grabbing;
  }

  .grid-backdrop {
    cursor: grab;
  }

  .path-hit {
    fill: none;
    stroke: transparent;
    stroke-width: 3;
    pointer-events: none;
  }

  .path-visible {
    fill: none;
    stroke: #f2f2f2;
    stroke-width: 0.28;
    pointer-events: none;
  }

  .point {
    fill: #0b0b0b;
    stroke: #f2f2f2;
    stroke-width: 0.2;
    cursor: move;
  }

  .point:hover,
  .control-point:hover {
    stroke-width: 0.45;
    filter: drop-shadow(0 0 0.8px #f2f2f2);
  }

  .point.endpoint {
    fill: #b6842a;
  }

  .control-point {
    fill: #1f7fe5;
    stroke: #0b3360;
    stroke-width: 0.2;
    cursor: move;
  }

  .control-line {
    stroke: #2f5c86;
    stroke-width: 0.18;
    stroke-dasharray: 0.8 0.8;
  }

  .controls {
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-height: 100%;
    overflow: auto;
    padding-right: 2px;
    height: 100%;
  }

  .panel {
    border: 1px solid #2a2a2a;
    padding: 10px;
    background: #0f0f0f;
    border-radius: 0;
    overflow-x: hidden;
  }

  .panel h2 {
    margin: 0 0 12px;
    font-size: 18px;
  }

  .segment-row,
  .point-row {
    display: grid;
    gap: 8px;
    margin-bottom: 12px;
  }

  .segment-actions {
    display: flex;
    justify-content: center;
    margin-bottom: 12px;
  }

  .segment-actions.end {
    margin-top: 8px;
    margin-bottom: 0;
  }

  .segment-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-weight: 600;
  }

  .segment-row select {
    padding: 4px 6px;
    background: #0b0b0b;
    color: #f2f2f2;
    border: 1px solid #2a2a2a;
    border-radius: 0;
  }

  .input-row,
  .point-inputs {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
    align-items: center;
  }

  .point-row {
    grid-template-columns: minmax(0, 1fr);
  }

  .point-row span {
    font-weight: 600;
  }

  .point-row label {
    display: grid;
    gap: 4px;
    font-size: 13px;
    letter-spacing: 0.2px;
    color: #b9b9b9;
    text-transform: uppercase;
  }

  .panel input[type='number'] {
    width: 100%;
    padding: 4px 6px;
    background: #0b0b0b;
    color: #f2f2f2;
    border: 1px solid #2a2a2a;
    border-radius: 0;
    box-sizing: border-box;
  }

  .panel button {
    padding: 6px 8px;
    border: 1px solid #2a2a2a;
    background: #0b0b0b;
    color: #f2f2f2;
    border-radius: 0;
    cursor: pointer;
  }

  textarea {
    width: 100%;
    resize: vertical;
    padding: 8px;
    border: 1px solid #2a2a2a;
    background: #0b0b0b;
    color: #f2f2f2;
    border-radius: 0;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono',
      'Courier New', monospace;
    box-sizing: border-box;
  }

  .remove {
    padding: 6px 8px;
    border: 1px solid #2a2a2a;
    background: #0b0b0b;
    color: #f2f2f2;
    border-radius: 0;
    cursor: pointer;
    justify-self: start;
  }

  .remove:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .axis-line {
    stroke: #565656;
    stroke-width: 0.25;
    pointer-events: none;
  }

  @media (max-width: 980px) {
    .layout {
      grid-template-columns: 1fr;
    }
  }
</style>
