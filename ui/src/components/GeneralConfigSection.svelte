<script lang="ts">
    import { onMount } from "svelte";
    import { api } from "../lib/api";

    let fetchSinceHours = 24;
    let imageTimeoutSeconds = 45;
    let loading = false;
    let message = "";

    onMount(async () => {
        await loadConfig();
    });

    async function loadConfig() {
        try {
            loading = true;
            const config = await api("/general-config");
            fetchSinceHours = config.fetch_since_hours;
            imageTimeoutSeconds = config.image_timeout_seconds;
        } catch (e: any) {
            message = "Failed to load config: " + e.message;
        } finally {
            loading = false;
        }
    }

    async function saveConfig() {
        try {
            loading = true;
            message = "";
            await api("/general-config", "POST", {
                fetch_since_hours: fetchSinceHours,
                image_timeout_seconds: imageTimeoutSeconds,
            });
            message = "Configuration saved successfully.";
        } catch (e: any) {
            message = "Failed to save config: " + e.message;
        } finally {
            loading = false;
        }
    }
</script>

<section class="card">
    <div class="card-header">
        <img
            src="/icons/settings.svg"
            alt="Settings Icon"
            width="20"
            height="20"
        />
        <h2>General Configuration</h2>
    </div>

    <div class="config-grid">
        <div class="form-group">
            <label for="fetch-since">Oldest RSS Article (hours)</label>
            <div class="input-group">
                <input
                    type="number"
                    id="fetch-since"
                    bind:value={fetchSinceHours}
                    min="1"
                />
            </div>
        </div>

        <div class="form-group">
            <label for="image-timeout">Image Processing Timeout (seconds)</label>
            <div class="input-group">
                <input
                    type="number"
                    id="image-timeout"
                    bind:value={imageTimeoutSeconds}
                    min="1"
                />
            </div>
        </div>
    </div>

    <div class="actions">
        <button on:click={saveConfig} disabled={loading} class="add-btn-modern">
            {loading ? "Saving..." : "Save Configuration"}
        </button>
        {#if message}
            <span class="message" class:error={message.includes("Failed")}
                >{message}</span
            >
        {/if}
    </div>
</section>

<style>
    .config-grid {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 1.5rem;
        margin-bottom: 0.5rem;
    }

    .form-group {
        margin-bottom: 0.5rem;
    }

    @media (max-width: 600px) {
        .config-grid {
            grid-template-columns: 1fr;
        }
    }

    label {
        display: block;
        margin-bottom: 0.5rem;
        color: var(--text-secondary);
    }

    .input-group {
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }

    input {
        width: 100%;
        padding: 0.75rem;
        border-radius: 6px;
        border: 1px solid var(--input-border);
        background: var(--input-bg);
        color: var(--text-primary);
    }

    .actions {
        display: flex;
        align-items: center;
        gap: 1rem;
    }

    .message {
        font-size: 0.9rem;
        color: var(--success);
    }

    .message.error {
        color: var(--error);
    }
</style>
