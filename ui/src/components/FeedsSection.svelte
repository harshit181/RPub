<script lang="ts">
    import { api } from "../lib/api";
    import { feeds, isAuthenticated, popup } from "../lib/store";
    import yaml from "js-yaml";

    let url = "";
    let name = "";
    let concurrency_limit: number | null = null;
    let processor = "default";  
    let customConfig = "";
    let customConfigError = "";
    let importStatus = "Import OPML";
    let isImporting = false;
    let fileInput: HTMLInputElement;
    
    let editModalOpen = false;
    let editingFeedId: number | null = null;
    let editingFeedName = "";
    let editProcessor = "default";
    let editCustomConfig = "";
    let editCustomConfigError = "";

    function validateYaml(value: string): string {
        if (!value.trim()) {
            return "Custom config cannot be empty";
        }
        try {
            const parsed = yaml.load(value);
            if (typeof parsed !== 'object' || parsed === null) {
                return "Config must be a YAML object";
            }
            if (!('selector' in parsed) || !Array.isArray((parsed as any).selector)) {
                return "Config must have 'selector' as an array";
            }
            return "";
        } catch (e: any) {
            return `Invalid YAML: ${e.message}`;
        }
    }

    $: if (processor === "custom") {
        customConfigError = validateYaml(customConfig);
    } else {
        customConfigError = "";
    }

    $: if (editProcessor === "custom") {
        editCustomConfigError = validateYaml(editCustomConfig);
    } else {
        editCustomConfigError = "";
    }

    $: isAddFormValid = processor !== "custom" || !customConfigError;

    $: if ($isAuthenticated) {
        loadFeeds();
    }

    async function loadFeeds() {
        try {
            const data = await api("/feeds");
            if (data) feeds.set(data);
        } catch (e) {
            console.error(e);
        }
    }

    async function addFeed() {
        if (!url) return;
        if (processor === "custom" && customConfigError) {
            popup.set({
                visible: true,
                title: "Validation Error",
                message: customConfigError,
                isError: true,
            });
            return;
        }
        try {
            await api("/feeds", "POST", {
                url,
                name: name || null,
                concurrency_limit: concurrency_limit || 0,
                processor: processor,
                custom_config: processor === "custom" ? customConfig : null,
            });
            url = "";
            name = "";
            concurrency_limit = null;
            processor = "default";
            customConfig = "";
            customConfigError = "";
            loadFeeds();
        } catch (e: any) {
            popup.set({
                visible: true,
                title: "Error",
                message: e.message,
                isError: true,
            });
        }
    }

    function deleteFeed(id: number) {
        popup.set({
            visible: true,
            title: "Confirm Deletion",
            message: "Delete this feed?",
            isError: false,
            type: "confirm",
            onConfirm: async () => {
                try {
                    await api(`/feeds/${id}`, "DELETE");
                    loadFeeds();
                } catch (e: any) {
                    popup.set({
                        visible: true,
                        title: "Error",
                        message: e.message,
                        isError: true,
                    });
                }
            },
            onCancel: () => {},
        });
    }

    async function openEditProcessor(feedId: number, feedName: string) {
        editingFeedId = feedId;
        editingFeedName = feedName;
        editModalOpen = true;
        
        try {
            const data = await api(`/feeds/${feedId}/processor`);
            if (data) {
                editProcessor = data.processor || "default";
                editCustomConfig = data.custom_config || "";
            } else {
                editProcessor = "default";
                editCustomConfig = "";
            }
        } catch (e) {
            editProcessor = "default";
            editCustomConfig = "";
        }
    }

    async function saveProcessor() {
        if (!editingFeedId) return;
        if (editProcessor === "custom" && editCustomConfigError) {
            popup.set({
                visible: true,
                title: "Validation Error",
                message: editCustomConfigError,
                isError: true,
            });
            return;
        }
        
        try {
            await api(`/feeds/${editingFeedId}/processor`, "PUT", {
                processor: editProcessor,
                custom_config: editProcessor === "custom" ? editCustomConfig : null,
            });
            editModalOpen = false;
            editingFeedId = null;
            editCustomConfigError = "";
            popup.set({
                visible: true,
                title: "Success",
                message: "Processor updated!",
                isError: false,
            });
        } catch (e: any) {
            popup.set({
                visible: true,
                title: "Error",
                message: e.message,
                isError: true,
            });
        }
    }

    function closeEditModal() {
        editModalOpen = false;
        editingFeedId = null;
        editProcessor = "default";
        editCustomConfig = "";
        editCustomConfigError = "";
    }

    async function importOpml(event: Event) {
        const input = event.target as HTMLInputElement;
        if (!input.files || input.files.length === 0) return;

        const file = input.files[0];
        const formData = new FormData();
        formData.append("file", file);

        isImporting = true;
        importStatus = "Importing...";

        try {
            const headers: Record<string, string> = {};

            const auth = localStorage.getItem("rsspub_auth");
            if (auth) headers["Authorization"] = auth;

            const res = await fetch("/feeds/import", {
                method: "POST",
                headers,
                body: formData,
            });

            if (res.ok) {
                popup.set({
                    visible: true,
                    title: "Success",
                    message: "Feeds imported successfully!",
                    isError: false,
                });
                loadFeeds();
            } else {
                throw new Error(await res.text());
            }
        } catch (e: any) {
            popup.set({
                visible: true,
                title: "Import Failed",
                message: e.message || "Unknown error",
                isError: true,
            });
        } finally {
            isImporting = false;
            importStatus = "Import OPML";
            input.value = "";
        }
    }
</script>

<section id="feeds-section" class="card">
    <div class="card-header">
        <img src="/icons/rss.svg" alt="Feed Icon" width="20" height="20" />
        <h2>Feeds</h2>
        <input
            type="file"
            bind:this={fileInput}
            id="opml-file"
            accept=".opml,.xml"
            style="display: none"
            on:change={importOpml}
        />
        <button
            on:click={() => fileInput.click()}
            class="add-btn"
            style="margin-left: auto; padding: 5px 10px; font-size: 0.8rem;"
            disabled={isImporting}
        >
            {importStatus}
        </button>
    </div>

    <ul id="feeds-list" class="item-list">
        {#each $feeds as feed (feed.id)}
            <li>
                <div style="display: flex; align-items: center; gap: 10px; flex: 1;">
                    <img
                        src="/icons/rss.svg"
                        alt="Feed Icon"
                        width="18"
                        height="18"
                        class="rss-icon"
                        style="filter: invert(36%) sepia(74%) saturate(836%) hue-rotate(185deg) brightness(97%) contrast(92%); flex-shrink: 0;"
                    />
                    <span>
                        {feed.name || feed.url}
                        <small
                            >({feed.concurrency_limit === 0
                                ? "Unlimited"
                                : feed.concurrency_limit + " threads"})</small
                        >
                    </span>
                </div>
                <div style="display: flex; gap: 8px; align-items: center;">
                    <button 
                        on:click={() => openEditProcessor(feed.id, feed.name || feed.url)} 
                        class="edit-btn"
                        title="Edit extractor"
                    >⚙</button>
                    <button on:click={() => deleteFeed(feed.id)} class="delete-btn">×</button>
                </div>
            </li>
        {/each}
    </ul>

    <form on:submit|preventDefault={addFeed} id="add-feed-form">
        <div class="input-group">
            <input
                type="url"
                bind:value={url}
                placeholder="Feed URL"
                required
            />
            <input
                type="text"
                bind:value={name}
                placeholder="Name (Optional)"
            />
        </div>
        <div class="input-group">
            <input
                type="number"
                bind:value={concurrency_limit}
                placeholder="Limit (0=Uni)"
                min="0"
            />
            <select bind:value={processor}>
                <option value="default">Default</option>
                <option value="dom_smoothie">DomSmoothie</option>
                <option value="custom">Custom (Experimental)</option>
            </select>
            <button type="submit" class="add-btn" disabled={!isAddFormValid}> Add Feed </button>
        </div>
        {#if processor === "custom"}
            <div class="input-group" style="margin-top: 10px;">
                <textarea
                    bind:value={customConfig}
                    placeholder="selector:
  - '.article-content'
discard:
  - '.ads'
output_mode: html"
                    rows="5"
                    style="width: 100%; font-family: monospace; font-size: 0.85rem;"
                    class:invalid={customConfigError}
                ></textarea>
            </div>
            {#if customConfigError}
                <div class="validation-error">{customConfigError}</div>
            {/if}
        {/if}
    </form>
</section>

<!-- Edit Processor Modal -->
{#if editModalOpen}
    <div class="modal-overlay" on:click={closeEditModal}>
        <div class="modal-box" on:click|stopPropagation>
            <h3>Edit Extractor: {editingFeedName}</h3>
            
            <div class="modal-field">
                <label>Processor Type</label>
                <select bind:value={editProcessor}>
                    <option value="default">Default</option>
                    <option value="dom_smoothie">DomSmoothie</option>
                    <option value="custom">Custom (Experimental)</option>
                </select>
            </div>
            
            {#if editProcessor === "custom"}
                <div class="modal-field">
                    <label>Custom Config (YAML)</label>
                    <textarea
                        bind:value={editCustomConfig}
                        placeholder="selector:
  - '.article-content'
discard:
  - '.ads'
output_mode: html"
                        rows="8"
                        style="font-family: monospace; font-size: 0.85rem;"
                        class:invalid={editCustomConfigError}
                    ></textarea>
                    {#if editCustomConfigError}
                        <div class="validation-error">{editCustomConfigError}</div>
                    {/if}
                </div>
            {/if}
            
            <div class="modal-actions">
                <button class="cancel-btn" on:click={closeEditModal}>Cancel</button>
                <button class="add-btn" on:click={saveProcessor} disabled={editProcessor === "custom" && !!editCustomConfigError}>Save</button>
            </div>
        </div>
    </div>
{/if}

<style>
    .edit-btn {
        background: transparent;
        color: var(--text-secondary);
        border: none;
        padding: 4px 8px;
        border-radius: 4px;
        cursor: pointer;
        font-size: 1rem;
    }
    .edit-btn:hover {
        background: rgba(59, 130, 246, 0.1);
        color: var(--accent-blue);
    }
    
    .modal-overlay {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.7);
        backdrop-filter: blur(5px);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 100;
    }
    
    .modal-box {
        background: var(--card-bg);
        border: 1px solid var(--card-border);
        border-radius: 16px;
        padding: 1.5rem;
        max-width: 500px;
        width: 90%;
        box-shadow: 0 10px 25px rgba(0, 0, 0, 0.5);
    }
    
    .modal-box h3 {
        margin: 0 0 1rem 0;
        color: var(--text-primary);
        font-size: 1.1rem;
    }
    
    .modal-field {
        margin-bottom: 1rem;
    }
    
    .modal-field label {
        display: block;
        margin-bottom: 0.5rem;
        color: var(--text-secondary);
        font-size: 0.9rem;
    }
    
    .modal-field select,
    .modal-field textarea {
        width: 100%;
        background: var(--input-bg);
        border: 1px solid var(--input-border);
        color: var(--text-primary);
        padding: 0.75rem;
        border-radius: 8px;
    }
    
    .modal-actions {
        display: flex;
        gap: 10px;
        justify-content: flex-end;
        margin-top: 1.5rem;
    }
    
    .cancel-btn {
        background: transparent;
        border: 1px solid var(--card-border);
        color: var(--text-secondary);
        padding: 0.5rem 1rem;
        border-radius: 6px;
        cursor: pointer;
    }
    
    .cancel-btn:hover {
        border-color: var(--text-secondary);
    }
    
    textarea {
        resize: vertical;
        background: var(--input-bg);
        border: 1px solid var(--input-border);
        color: var(--text-primary);
        border-radius: 8px;
        padding: 0.75rem;
    }
    
    textarea.invalid {
        border-color: var(--danger);
        background: rgba(239, 68, 68, 0.1);
        color: var(--text-primary);
    }
    
    .validation-error {
        color: var(--text-primary);
        font-size: 0.8rem;
        margin-top: 0.5rem;
        padding: 0.5rem;
        background: rgba(239, 68, 68, 0.15);
        border-radius: 4px;
        border-left: 3px solid var(--danger);
    }
    
    button:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }
</style>
