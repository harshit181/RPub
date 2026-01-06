<script lang="ts">
    import { popup } from "../lib/store";

    function hidePopup() {
        popup.update((p) => ({ ...p, visible: false }));
    }

    function onConfirm() {
        if ($popup.type === "confirm") {
            $popup.onConfirm();
        }
        hidePopup();
    }

    function onCancel() {
        if ($popup.type === "confirm") {
            $popup.onCancel();
        }
        hidePopup();
    }
</script>

{#if $popup.visible}
    <div id="popup-overlay">
        <div class="popup-box">
            <h3 class="popup-title">{$popup.title || "Confirm Action"}</h3>
            <p class="popup-message">{$popup.message}</p>
            <div class="popup-actions">
                {#if $popup.type === "confirm"}
                    <button class="popup-btn secondary" on:click={onCancel}>Cancel</button>
                    <button class="popup-btn primary" on:click={onConfirm}>OK</button>
                {:else}
                    <button
                        class="popup-btn"
                        class:error={$popup.isError}
                        on:click={hidePopup}>OK</button
                    >
                {/if}
            </div>
        </div>
    </div>
{/if}
