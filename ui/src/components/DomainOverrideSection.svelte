<script lang="ts">
    import { api } from "../lib/api";
    import { isAuthenticated, popup } from "../lib/store";
    import yaml from "js-yaml";

    interface DomainOverride {
        id: number;
        domain: string;
        processor: string;
        custom_config: string | null;
        created_at: string;
    }

    let overrides: DomainOverride[] = [];
    let domain = "";
    let processor = "default";
    let customConfig = "";
    let customConfigError = "";
    let loading = false;

    const processorOptions = [
        { value: "default", label: "Default" },
        { value: "dom_smoothie", label: "DomSmoothie" },
        { value: "text_only", label: "Text Only (No Images)" },
        { value: "custom", label: "Custom (Experimental)" },
    ];

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

    $: isAddFormValid = processor !== "custom" || !customConfigError;

    $: if ($isAuthenticated) {
        loadOverrides();
    }

    async function loadOverrides() {
        try {
            loading = true;
            const data = await api("/domain-overrides");
            if (data) overrides = data;
        } catch (e) {
            console.error(e);
        } finally {
            loading = false;
        }
    }

    async function addOverride() {
        if (!domain.trim()) {
            popup.set({
                visible: true,
                title: "Validation Error",
                message: "Domain cannot be empty",
                isError: true,
            });
            return;
        }

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
            await api("/domain-overrides", "POST", {
                domain: domain.trim().toLowerCase(),
                processor,
                custom_config: processor === "custom" ? customConfig : null,
            });
            domain = "";
            processor = "default";
            customConfig = "";
            customConfigError = "";
            loadOverrides();
            popup.set({
                visible: true,
                title: "Success",
                message: "Domain override added!",
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

    function deleteOverride(id: number, domainName: string) {
        popup.set({
            visible: true,
            title: "Confirm Deletion",
            message: `Delete override for "${domainName}"?`,
            isError: false,
            type: "confirm",
            onConfirm: async () => {
                try {
                    await api(`/domain-overrides/${id}`, "DELETE");
                    loadOverrides();
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

    function getProcessorLabel(value: string): string {
        const option = processorOptions.find(o => o.value === value);
        return option ? option.label : value;
    }
</script>

<section class="card">
    <div class="card-header">
        <img src="/icons/settings.svg" alt="Domain Icon" width="20" height="20" />
        <h2>Domain Overrides</h2>
    </div>

    <p class="section-description">
        Configure processor types for specific domains. When fetching content from matching domains, the specified processor will be used instead of the read later articles and feed default.
    </p>

    <ul class="item-list">
        {#each overrides as override (override.id)}
            <li>
                <div style="display: flex; align-items: center; gap: 10px; flex: 1;">
                    <img
                        src="/icons/settings.svg"
                        alt="Domain Icon"
                        width="18"
                        height="18"
                        style="filter: invert(36%) sepia(74%) saturate(836%) hue-rotate(185deg) brightness(97%) contrast(92%); flex-shrink: 0;"
                    />
                    <span>
                        <strong>{override.domain}</strong>
                        <small>→ {getProcessorLabel(override.processor)}</small>
                        {#if override.custom_config}
                            <small title={override.custom_config}> (with config)</small>
                        {/if}
                    </span>
                </div>
                <button on:click={() => deleteOverride(override.id, override.domain)} class="delete-btn">×</button>
            </li>
        {:else}
            {#if !loading}
                <li class="empty-state">No domain overrides configured</li>
            {/if}
        {/each}
    </ul>

    <form on:submit|preventDefault={addOverride}>
        <div class="input-group">
            <input
                type="text"
                bind:value={domain}
                placeholder="Domain (e.g., example.com)"
                required
            />
            <select bind:value={processor}>
                {#each processorOptions as option}
                    <option value={option.value}>{option.label}</option>
                {/each}
            </select>
            <button type="submit" class="add-btn" disabled={!isAddFormValid}>Add Override</button>
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
