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

    function deleteArticle(id: number) {
        popup.set({
            visible: true,
            title: "Confirm Deletion",
            message: "Are you sure you want to delete this article?",
            isError: false,
            type: "confirm",
            onConfirm: async () => {
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
            },
            onCancel: () => {},
        });
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
    <div class="ril-header">
        <h2>Read It Later</h2>
        <button class="add-btn-modern" on:click={deliverNow} disabled={delivering}>
            {delivering ? "Delivering..." : "Deliver it now"}
        </button>
    </div>

    <div class="ril-add-form">
        <input type="text" placeholder="URL" bind:value={url} />
        <button class="btn secondary" on:click={addArticle} disabled={loading || !url}>
            {loading ? "Adding..." : "Add"}
        </button>
    </div>

    <div class="ril-list">
        {#each articles as article}
            <div class="ril-item {article.read ? 'read' : ''}">
                <div class="ril-info">
                    <div class="ril-title">{article.url}</div>
                    <div class="ril-meta">{new Date(article.created_at).toLocaleString()}</div>
                </div>
                <div class="ril-actions">
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
            <div class="ril-empty">No articles saved.</div>
        {/if}
    </div>
</div>
