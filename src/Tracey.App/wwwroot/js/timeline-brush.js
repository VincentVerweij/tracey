// Timeline overview brush (#125, research #109): drag the zoom-range rectangle
// on the overview bar to pan, drag an edge to resize, press outside it to
// centre there and keep dragging.
//
// JS owns the drag: it sets pointer capture on pointerdown and previews each
// move through requestAnimationFrame, with no .NET call. Pointerup or
// pointercancel makes one CommitBrush call; .NET re-applies the ZoomRange
// rules and pushes the result back through setZoom after it renders.
//
// Everything that follows the range lives in what Blazor never renders, so
// the two never fight over the DOM: on the page root, the --zoom-start and
// --zoom-length custom properties (fractions of the day) and the data-zoom-*
// data-brush and aria-busy attributes; and the text of the empty [data-zoom-text] elements.
//
// The gesture rules mirror ZoomRange (TimelineModel/ZoomRange.cs) so the
// preview doesn't overshoot what .NET commits. Times here are minutes.
(function () {
    'use strict';

    const DAY = 24 * 60;
    const MIN_LENGTH = 15;
    const FULL_DAY_SNAP = 1;
    const GRAB_PX = 8;
    const NARROW_PX = 24;

    const _pages = new Map();

    function init(root, dotNetRef) {
        const page = {
            root,
            dotNetRef,
            committed: { start: 0, length: DAY, text: '00:00–24:00' },
            drag: null,
            frame: 0,
            resize: new ResizeObserver(() => { if (!page.drag) apply(page, page.committed); }),
        };
        page.listeners = {
            pointerdown: e => onPointerDown(page, e),
            pointermove: e => onPointerMove(page, e),
            pointerup: e => onPointerEnd(page, e),
            pointercancel: e => onPointerEnd(page, e),
            lostpointercapture: e => onPointerEnd(page, e),
        };
        for (const [name, fn] of Object.entries(page.listeners)) root.addEventListener(name, fn);
        page.resize.observe(root);
        _pages.set(root, page);
        apply(page, page.committed);
    }

    function dispose(root) {
        const page = _pages.get(root);
        if (!page) return;
        for (const [name, fn] of Object.entries(page.listeners)) root.removeEventListener(name, fn);
        page.resize.disconnect();
        cancelAnimationFrame(page.frame);
        _pages.delete(root);
    }

    // Called by .NET after every render with the authoritative range. A drag in
    // progress keeps its preview; its commit brings the next setZoom.
    function setZoom(root, start, length, text) {
        const page = _pages.get(root);
        if (!page) return;
        page.committed = { start, length, text };
        if (!page.drag) apply(page, page.committed);
    }

    function isBrushing() {
        for (const page of _pages.values()) if (page.drag) return true;
        return false;
    }

    // ── Pointer handling ─────────────────────────────────────────────────

    function onPointerDown(page, e) {
        const overview = e.target.closest('.timeline-overview');
        if (!overview || page.drag || !e.isPrimary || e.button !== 0) return;
        const box = overview.getBoundingClientRect();
        if (box.width <= 0) return;
        e.preventDefault();
        overview.setPointerCapture(e.pointerId);
        const from = e.clientX - box.left;
        const range = page.committed;
        const hit = hitTest(range.start / DAY * box.width, range.length / DAY * box.width, from);
        page.drag = { pointerId: e.pointerId, left: box.left, width: box.width, from, to: from, range, hit };
        page.root.dataset.brush = hit === 'left' || hit === 'right' ? 'resize' : 'pan';
        // The range text rewrites every frame; announce only the result.
        page.root.setAttribute('aria-busy', 'true');
        // A press outside centres at once, before any move.
        schedulePreview(page);
    }

    function onPointerMove(page, e) {
        const drag = page.drag;
        if (!drag || e.pointerId !== drag.pointerId) return;
        drag.to = e.clientX - drag.left;
        schedulePreview(page);
    }

    function onPointerEnd(page, e) {
        const drag = page.drag;
        if (!drag || e.pointerId !== drag.pointerId) return;
        page.drag = null;
        cancelAnimationFrame(page.frame);
        page.frame = 0;
        delete page.root.dataset.brush;
        page.root.removeAttribute('aria-busy');
        // The range the press was hit-tested against, so the commit matches the preview.
        const { start, length } = drag.range;
        page.dotNetRef.invokeMethodAsync('CommitBrush', start, length, drag.width, drag.from, drag.to)
            .catch(err => {
                console.error('[timeline-brush] CommitBrush failed', err);
                apply(page, page.committed);
            });
    }

    function schedulePreview(page) {
        if (page.frame) return;
        page.frame = requestAnimationFrame(() => {
            page.frame = 0;
            const drag = page.drag;
            if (drag) apply(page, brush(drag.range, drag.hit, drag.width, drag.from, drag.to));
        });
    }

    // ── Rendering ────────────────────────────────────────────────────────

    function apply(page, range) {
        const root = page.root;
        root.style.setProperty('--zoom-start', String(range.start / DAY));
        root.style.setProperty('--zoom-length', String(range.length / DAY));
        root.toggleAttribute('data-zoom-full', range.length >= DAY);
        const overview = root.querySelector('.timeline-overview');
        const width = overview ? overview.clientWidth : 0;
        root.toggleAttribute('data-zoom-narrow', width > 0 && range.length / DAY * width < NARROW_PX);
        const text = range.text ?? rangeText(range);
        for (const el of root.querySelectorAll('[data-zoom-text]'))
            if (el.textContent !== text) el.textContent = text;
    }

    function rangeText(range) {
        return `${clock(range.start)}–${clock(range.start + range.length)}`;
    }

    // Rounds halves to even, like C#'s Math.Round in ZoomRange.Clock.
    function clock(minutes) {
        const floor = Math.floor(minutes);
        const m = minutes - floor === 0.5 ? floor + (floor % 2) : Math.round(minutes);
        return `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`;
    }

    // ── Gesture rules, mirroring ZoomRange ───────────────────────────────

    function hitTest(left, width, x) {
        const right = left + width;
        if (width < NARROW_PX) {
            if (x >= left - GRAB_PX && x < left) return 'left';
            if (x > right && x <= right + GRAB_PX) return 'right';
            return x >= left && x <= right ? 'body' : 'outside';
        }
        const half = GRAB_PX / 2;
        if (x >= left - half && x < left + half) return 'left';
        if (x > right - half && x <= right + half) return 'right';
        return x >= left && x <= right ? 'body' : 'outside';
    }

    function brush(range, hit, width, from, to) {
        const perPx = DAY / width;
        const delta = perPx * (to - from);
        const end = range.start + range.length;
        switch (hit) {
            case 'body': return create(range.start + delta, range.length);
            case 'left': {
                const start = clamp(range.start + delta, 0, end - MIN_LENGTH);
                return create(start, end - start);
            }
            case 'right': {
                const newEnd = clamp(end + delta, range.start + MIN_LENGTH, DAY);
                return create(range.start, newEnd - range.start);
            }
            default: {
                const centred = create(perPx * from - range.length / 2, range.length);
                return create(centred.start + delta, centred.length);
            }
        }
    }

    function create(start, length) {
        if (length >= DAY - FULL_DAY_SNAP) return { start: 0, length: DAY };
        length = Math.max(length, MIN_LENGTH);
        return { start: clamp(start, 0, DAY - length), length };
    }

    function clamp(value, min, max) {
        return Math.min(Math.max(value, min), max);
    }

    window.traceyTimeline = { init, dispose, setZoom, isBrushing };
})();
