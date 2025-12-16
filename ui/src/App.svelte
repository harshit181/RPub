<script lang="ts">
    import Header from "./components/Header.svelte";
    import LoginOverlay from "./components/LoginOverlay.svelte";
    import Popup from "./components/Popup.svelte";
    import FeedsSection from "./components/FeedsSection.svelte";
    import GenerationSection from "./components/GenerationSection.svelte";
    import CoverSection from "./components/CoverSection.svelte";
    import SchedulesSection from "./components/SchedulesSection.svelte";
    import DownloadsSection from "./components/DownloadsSection.svelte";
    import EmailConfigSection from "./components/EmailConfigSection.svelte";
    import ReadItLaterSection from "./components/ReadItLaterSection.svelte";
    import Tabs from "./components/Tabs.svelte";
    import { onMount } from "svelte";
    import { api } from "./lib/api";
    import { isAuthenticated, authHeader, isLoginVisible } from "./lib/store";

    let activeTab = "Dashboard";
    const tabs = ["Dashboard", "Configuration", "Read It Later"];

    onMount(async () => {
        window.addEventListener("unauthorized", () => {
            isAuthenticated.set(false);
            authHeader.set(null);
            isLoginVisible.set(true);
        });

        try {
            await api("/auth/check");
            isAuthenticated.set(true);
        } catch (e) {
            isAuthenticated.set(false);
            authHeader.set(null);
            isLoginVisible.set(true);
        }
    });
</script>

<LoginOverlay />
<Popup />

{#if $isAuthenticated}
    <div class="container">
        <Header />

        <div class="tabs-container">
            <Tabs {tabs} bind:activeTab />
        </div>

        {#if activeTab === "Dashboard"}
            <main class="dashboard-grid">
                <div class="column left-col">
                    <FeedsSection />
                </div>

                <div class="column right-col">
                    <GenerationSection />
                    <DownloadsSection />
                </div>
            </main>
        {:else if activeTab === "Configuration"}
            <main class="dashboard-grid">
                <div class="column left-col">
                    <SchedulesSection />
                    <EmailConfigSection />
                </div>
                <div class="column right-col">
                    <CoverSection />
                </div>
            </main>
        {:else if activeTab === "Read It Later"}
            <main class="dashboard-grid">
                <div class="column left-col">
                    <ReadItLaterSection />
                </div>
            </main>
        {/if}
    </div>
{:else}
    <!-- -->
{/if}

<style>
</style>
