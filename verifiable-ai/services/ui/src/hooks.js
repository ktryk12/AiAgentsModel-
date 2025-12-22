import { useEffect, useRef, useState } from "react";

// Helper to keep a value stable if deep equal (simple version) or just use ref
function useStableDeps(deps) {
    const ref = useRef(deps);
    if (JSON.stringify(ref.current) !== JSON.stringify(deps)) {
        ref.current = deps;
    }
    return ref.current;
}

export function usePoll(fn, intervalMs, rawDeps = []) {
    const [data, setData] = useState(null);
    const [error, setError] = useState(null);
    const [loading, setLoading] = useState(true);

    // Ensure deps are stable to prevent effect re-running on every render
    const deps = useStableDeps(rawDeps);

    useEffect(() => {
        let timer = null;
        let alive = true;

        async function tick() {
            if (document.visibilityState === "hidden") return;
            try {
                const v = await fn();
                if (alive) {
                    setData(v);
                    setError(null);
                    setLoading(false);
                }
            } catch (e) {
                if (alive) {
                    setError(e);
                    setLoading(false);
                }
            }
        }

        function onVisChange() {
            if (document.visibilityState === "visible") {
                tick();
                // restart interval to align with visibility
                clearInterval(timer);
                timer = setInterval(tick, intervalMs);
            }
        }

        tick();
        timer = setInterval(tick, intervalMs);
        document.addEventListener("visibilitychange", onVisChange);

        return () => {
            alive = false;
            clearInterval(timer);
            document.removeEventListener("visibilitychange", onVisChange);
        };
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [intervalMs, deps]); // fn is omitted as it might be unstable arrow function

    return { data, error, loading, refresh: fn };
}
