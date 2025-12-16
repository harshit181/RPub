<script lang="ts">
    import { onMount } from "svelte";
    import { api } from "../lib/api";
    import { schedules, isAuthenticated, popup } from "../lib/store";
    let hour = "";
    let minute = "";
    let scheduleType = "rss";
    let timezone = Intl.DateTimeFormat().resolvedOptions().timeZone;

    const hours = Array.from({ length: 24 }, (_, i) =>
        i.toString().padStart(2, "0"),
    );
    const minutes = Array.from({ length: 12 }, (_, i) =>
        (i * 5).toString().padStart(2, "0"),
    );
    const timezones = Intl.supportedValuesOf("timeZone");

    onMount(() => {
        const localTz = Intl.DateTimeFormat().resolvedOptions().timeZone;
        if (timezones.includes(localTz)) {
            timezone = localTz;
        }
    });

    $: if ($isAuthenticated) {
        loadSchedules();
    }

    async function loadSchedules() {
        try {
            const data = await api("/schedules");
            if (data) schedules.set(data);
        } catch (e) {
            console.error(e);
        }
    }

    async function addSchedule() {
        if (!hour || !minute || !timezone || !scheduleType) {
            popup.set({
                visible: true,
                title: "Missing Information",
                message: "Please select Hour, Minute, Timezone, and Type.",
                isError: true,
            });
            return;
        }

        try {
            await api("/schedules", "POST", {
                hour: parseInt(hour, 10),
                minute: parseInt(minute, 10),
                timezone,
                schedule_type: scheduleType,
            });
            loadSchedules();
        } catch (e: any) {
            popup.set({
                visible: true,
                title: "Error",
                message: e.message,
                isError: true,
            });
        }
    }

    async function deleteSchedule(id: number) {
        if (!confirm("Delete this schedule?")) return;
        try {
            await api(`/schedules/${id}`, "DELETE");
            loadSchedules();
        } catch (e: any) {
            popup.set({
                visible: true,
                title: "Error",
                message: e.message,
                isError: true,
            });
        }
    }

    function formatSchedule(timeStr: string) {
        try {
            const date = new Date(timeStr);
            return new Intl.DateTimeFormat("default", {
                hour: "numeric",
                minute: "numeric",
                timeZoneName: "short",
            }).format(date);
        } catch (e) {
            return timeStr;
        }
    }
</script>

<section id="schedules-section" class="card">
    <div class="card-header">
        <img
            src="/icons/clock.svg"
            alt="Schedule Icon"
            width="20"
            height="20"
        />
        <h2>Schedules</h2>
    </div>
    <ul id="schedules-list" class="item-list">
        {#each $schedules as schedule (schedule.id)}
            <li>
                <div class="schedule-info">
                    <span class="schedule-time">{formatSchedule(schedule.time)}</span>
                    <span class="schedule-type-badge">{schedule.schedule_type || 'rss'}</span>
                </div>
                <button
                    on:click={() => deleteSchedule(schedule.id)}
                    class="delete-btn">×</button
                >
            </li>
        {/each}
    </ul>
    <form on:submit|preventDefault={addSchedule} id="add-schedule-form">
        <div class="time-row">
            <select bind:value={hour} required>
                <option value="" disabled selected>00</option>
                {#each hours as h}
                    <option value={h}>{h}</option>
                {/each}
            </select>
            <span>:</span>
            <select bind:value={minute} required>
                <option value="" disabled selected>00</option>
                {#each minutes as m}
                    <option value={m}>{m}</option>
                {/each}
            </select>
            <select bind:value={timezone} id="timezone-select" required>
                {#each timezones as tz}
                    <option value={tz}>{tz}</option>
                {/each}
            </select>
            <select bind:value={scheduleType} required>
                <option value="rss">RSS Generator</option>
                <option value="read_it_later">Read It Later</option>
            </select>
            <button type="submit" class="add-btn"> Add Schedule </button>
        </div>
    </form>
</section>
