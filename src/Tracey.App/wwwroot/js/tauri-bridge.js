// Tauri v2 event bridge — connects Rust backend events to Blazor C# via DotNetObjectReference
(function () {
    'use strict';

    let _dotNetRef = null;
    const _unlisten = [];

    async function initializeTauriBridge(dotNetRef) {
        console.log('[tracey-bridge] initializeTauriBridge called');
        _dotNetRef = dotNetRef;

        const events = [
            'tracey://timer-tick',
            'tracey://idle-detected',
            'tracey://idle-resolved',
            'tracey://screenshot-captured',
            'tracey://sync-status-changed',
            'tracey://notification-sent',
            'tracey://error'
        ];

        for (const eventName of events) {
            try {
                // Tauri v2: listen is not on __TAURI_INTERNALS__ directly.
                // Use transformCallback (creates a persistent window-level handler)
                // then invoke the plugin:event|listen command.
                const handlerId = window.__TAURI_INTERNALS__.transformCallback((event) => {
                    console.log('[tracey-bridge] EVENT RECEIVED:', eventName, event.payload ?? event);
                    if (_dotNetRef) {
                        _dotNetRef.invokeMethodAsync('RouteEvent', eventName, JSON.stringify(event.payload ?? event))
                            .catch((err) => console.error('[tracey-bridge] RouteEvent failed for', eventName, err));
                    } else {
                        console.warn('[tracey-bridge] EVENT RECEIVED but _dotNetRef is null — bridge disposed?', eventName);
                    }
                }, false); // false = keepAlive (not a one-shot callback)

                const eventId = await window.__TAURI_INTERNALS__.invoke('plugin:event|listen', {
                    event: eventName,
                    target: { kind: 'Any' },
                    handler: handlerId
                });

                console.log('[tracey-bridge] Registered listener for', eventName, '→ eventId', eventId);
                _unlisten.push(() => {
                    window.__TAURI_INTERNALS__.invoke('plugin:event|unlisten', {
                        event: eventName,
                        eventId: eventId
                    }).catch(() => {});
                });
            } catch (e) {
                console.error('[tracey-bridge] FAILED to register listener for', eventName, e);
            }
        }

        console.log('[tracey-bridge] Bridge initialised — subscribed to', events.length, 'events');
    }

    function disposeTauriBridge() {
        _unlisten.forEach(fn => { try { fn(); } catch (_) {} });
        _unlisten.length = 0;
        _dotNetRef = null;
    }

    // Convert a local filesystem path to the correct Tauri v2 asset URL for WebView2 on Windows.
    // Tauri v2 on Windows uses https://asset.localhost with URL-encoded drive colon.
    // Example: C:\users\foo\screenshots\bar.jpg → https://asset.localhost/C%3A/users/foo/screenshots/bar.jpg
    function convertFileSrc(filePath) {
        if (typeof filePath !== 'string' || !filePath) return '';
        // Try Tauri's built-in convertFileSrc if available
        if (window.__TAURI_INTERNALS__?.convertFileSrc) {
            return window.__TAURI_INTERNALS__.convertFileSrc(filePath);
        }
        // Manual fallback for Windows paths
        const normalized = filePath.replace(/\\/g, '/').replace(/^\/+/, '');
        // URL-encode the colon after a Windows drive letter: C: → C%3A
        const encoded = normalized.replace(/^([A-Za-z]):/, '$1%3A');
        return 'https://asset.localhost/' + encoded;
    }

    function getTimelineBarWidth() {
        return document.querySelector('.timeline-bar-inner')?.clientWidth ?? 800;
    }

    window.traceyBridge = { initializeTauriBridge, disposeTauriBridge, convertFileSrc, getTimelineBarWidth };
})();
