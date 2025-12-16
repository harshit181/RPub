<script lang="ts">
    import { onMount } from "svelte";
    import { api } from "../lib/api";
    import {popup} from "../lib/store";

    let url = "";
    let articles: any[] = [];
    let loading = false;
    let delivering = false;

    async function loadArticles() {
        try {
            articles = await api("/read-it-later");
        } catch (e) {
            console.error(e);
            popup.set({
                visible: true,
                title: "Failed to load articles",
                message: "Failed to load articles",
                isError: true,
            });
        }
    }

    async function addArticle() {
        if (!url) return;
        loading = true;
        try {
            await api("/read-it-later", "POST", { url });
            url = "";
            await loadArticles();
        } catch (e) {
            console.error(e);
            popup.set({
                visible: true,
                title: "Failed to add articles",
                message: "Failed to add articles",
                isError: true,
            });
        } finally {
            loading = false;
        }
    }

    async function toggleRead(id: number, currentStatus: boolean) {
        try {
            await api(`/read-it-later/${id}`, "PATCH", { read: !currentStatus });
            await loadArticles();
        } catch (e) {
            console.error(e);
            popup.set({
                visible: true,
                title: "Failed to update status",
                message: "Failed to update status",
                isError: true,
            });
        }
    }

    async function deleteArticle(id: number) {
        if (!confirm("Are you sure?")) return;
        try {
            await api(`/read-it-later/${id}`, "DELETE");
            await loadArticles();
        } catch (e) {
            console.error(e);
            popup.set({
                visible: true,
                title: "Failed to delete articles",
                message: "Failed to delete articles",
                isError: true,
            });
        }
    }

    async function deliverNow() {
        delivering = true;
        try {
            await api("/read-it-later/deliver", "POST");
            popup.set({
                visible: true,
                title: "Generation started",
                message: "Generation started!",
                isError: false,
            });
        } catch (e) {
            console.error(e);
            popup.set({
                visible: true,
                title: "Generation failed",
                message: "Generation failed!",
                isError: true,
            });
        } finally {
            delivering = false;
        }
    }

    onMount(loadArticles);
</script>

<div class="card">
    <div class="header">
        <h2>Read It Later</h2>
        <button class="btn primary" on:click={deliverNow} disabled={delivering}>
            {delivering ? "Delivering..." : "Deliver it now"}
        </button>
    </div>

    <div class="add-form">
        <input type="text" placeholder="URL" bind:value={url} />
        <button class="btn secondary" on:click={addArticle} disabled={loading || !url}>
            {loading ? "Adding..." : "Add"}
        </button>
    </div>

    <div class="list">
        {#each articles as article}
            <div class="item {article.read ? 'read' : ''}">
                <div class="info">
                    <div class="title">{article.url}</div>
                    <div class="meta">{new Date(article.created_at).toLocaleString()}</div>
                </div>
                <div class="actions">
                    <button class="btn text" on:click={() => toggleRead(article.id, article.read)}>
                        {article.read ? "Mark Unread" : "Mark Read"}
                    </button>
                    <button class="btn text danger" on:click={() => deleteArticle(article.id)}>
                        Delete
                    </button>
                </div>
            </div>
        {/each}
        {#if articles.length === 0}
            <div class="empty">No articles saved.</div>
        {/if}
    </div>
</div>

<style>
    .card {
        background: var(--card-bg);
        border: 1px solid var(--card-border);
        border-radius: 12px;
        padding: 1.5rem;
    }

    .header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 1.5rem;
    }

    h2 {
        margin: 0;
        font-size: 1.25rem;
        font-weight: 600;
    }

    .add-form {
        display: flex;
        gap: 0.5rem;
        margin-bottom: 1.5rem;
    }

    input {
        flex: 1;
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid var(--card-border);
        border-radius: 6px;
        padding: 0.5rem 0.75rem;
        color: var(--text-primary);
    }

    .list {
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
    }

    .item {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.75rem;
        background: rgba(255, 255, 255, 0.02);
        border-radius: 8px;
        border: 1px solid transparent;
    }

    .item.read {
        opacity: 0.6;
    }

    .info {
        flex: 1;
        min-width: 0;
    }

    .title {
        font-weight: 500;
        margin-bottom: 0.25rem;
        word-break: break-all;
    }

    .meta {
        font-size: 0.85rem;
        color: var(--text-secondary);
    }

    .actions {
        display: flex;
        gap: 0.5rem;
    }

    .btn {
        padding: 0.5rem 1rem;
        border-radius: 6px;
        font-weight: 500;
        cursor: pointer;
        transition: all 0.2s;
        border: none;
    }

    .btn.primary {
        background: var(--accent-blue);
        color: white;
    }

    .btn.primary:hover {
        background: var(--accent-blue-hover);
    }

    .btn.secondary {
        background: rgba(255, 255, 255, 0.1);
        color: var(--text-primary);
    }

    .btn.secondary:hover {
        background: rgba(255, 255, 255, 0.15);
    }

    .btn.text {
        background: transparent;
        color: var(--text-secondary);
        padding: 0.25rem 0.5rem;
    }

    .btn.text:hover {
        color: var(--text-primary);
        background: rgba(255, 255, 255, 0.05);
    }

    .btn.danger {
        color: #ef4444;
    }

    .btn.danger:hover {
        background: rgba(239, 68, 68, 0.1);
    }

    .empty {
        text-align: center;
        color: var(--text-secondary);
        padding: 2rem;
    }

    button:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }
</style>
