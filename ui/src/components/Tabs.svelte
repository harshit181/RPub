<script lang="ts">
    export let tabs: string[] = [];
    export let activeTab: string = "";

    function selectTab(tab: string) {
        activeTab = tab;
    }
</script>

<div class="tabs">
    {#each tabs as tab}
        <button
            class="tab-btn {activeTab === tab ? 'active' : ''}"
            on:click={() => selectTab(tab)}
        >
            {tab}
        </button>
    {/each}
</div>
