<script lang="ts">
    import { api } from "../lib/api";
    import { downloads } from "../lib/store";

    let isGenerating = false;
    let status = "";

    async function generate() {
        if (isGenerating) return;
        isGenerating = true;
        status = "Requesting generation...";

        try {
            await api("/generate", "POST", {
                feeds: []
            });
            status = "Generation started in background. Please wait...";

            let checks = 0;
            const interval = setInterval(async () => {
                checks++;
                try {
                    const data = await api("/downloads");
                    if (data) downloads.set(data);
                } catch (e) {}

                if (checks > 10) {
                    clearInterval(interval);
                    status = "Generation started. Check downloads list below.";
                    isGenerating = false;
                }
            }, 2000);
        } catch (e: any) {
            status = "Error: " + e.message;
            isGenerating = false;
        }
    }
</script>

<section id="generate-section" class="card">
    <div class="card-header">
        <img src="/icons/zap.svg" alt="Generate Icon" width="20" height="20" />
        <h2>Manual Generation</h2>
    </div>
    <div class="generate-wrapper">
        <button id="generate-btn" on:click={generate} disabled={isGenerating}
            >Generate EPUB Now</button
        >
        <div id="status">{status}</div>
    </div>
</section>
