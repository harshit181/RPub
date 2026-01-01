<script lang="ts">
    import { isAuthenticated } from "../lib/store";
    import { onMount } from "svelte";

    let isSketchTheme = false;

    onMount(() => {
        const savedTheme = localStorage.getItem("rsspub-theme");
        if (savedTheme === "sketch") {
            isSketchTheme = true;
            document.body.classList.add("sketch-theme");
        }
    });

    function toggleTheme() {
        isSketchTheme = !isSketchTheme;
        if (isSketchTheme) {
            document.body.classList.add("sketch-theme");
            localStorage.setItem("rsspub-theme", "sketch");
        } else {
            document.body.classList.remove("sketch-theme");
            localStorage.setItem("rsspub-theme", "default");
        }
    }
</script>

<header>
    <div class="header-content">
        <h1>RSSPub</h1>
        <p>RSS Aggregator & EPUB Generator</p>
    </div>
    <div class="status-indicator">
        <img
            src="/icons/book.svg"
            alt="Logo"
            width="18"
            height="18"
            style="margin-right: 6px; filter: invert(36%) sepia(74%) saturate(836%) hue-rotate(185deg) brightness(97%) contrast(92%);"
        />
        <span id="connection-status"
            >{$isAuthenticated ? "Connected" : "Disconnected"}</span
        >
        <button class="theme-toggle" on:click={toggleTheme} title="Toggle theme">
            {#if isSketchTheme}
                <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <circle cx="12" cy="12" r="5"/>
                    <line x1="12" y1="1" x2="12" y2="3"/>
                    <line x1="12" y1="21" x2="12" y2="23"/>
                    <line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/>
                    <line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/>
                    <line x1="1" y1="12" x2="3" y2="12"/>
                    <line x1="21" y1="12" x2="23" y2="12"/>
                    <line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/>
                    <line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/>
                </svg>
                <span>Default</span>
            {:else}
                <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M12 19l7-7 3 3-7 7-3-3z"/>
                    <path d="M18 13l-1.5-7.5L2 2l3.5 14.5L13 18l5-5z"/>
                    <path d="M2 2l7.586 7.586"/>
                    <circle cx="11" cy="11" r="2"/>
                </svg>
                <span>Sketch</span>
            {/if}
        </button>
    </div>
</header>
