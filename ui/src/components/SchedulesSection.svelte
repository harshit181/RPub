<script lang="ts">
    import { onMount } from "svelte";
    import { api } from "../lib/api";
    import { schedules, isAuthenticated, popup } from "../lib/store";
    let hour = "";
    let minute = "";
    let scheduleType = "rss";
    let frequency = "daily";
    let dayOfWeek = "0";
    let dayOfMonth = "1";
    let timezone = Intl.DateTimeFormat().resolvedOptions().timeZone;

    const hours = Array.from({ length: 24 }, (_, i) =>
        i.toString().padStart(2, "0"),
    );
    const minutes = Array.from({ length: 12 }, (_, i) =>
        (i * 5).toString().padStart(2, "0"),
    );
    const timezones = Intl.supportedValuesOf("timeZone");

    const daysOfWeek = [
        { val: "0", label: "Monday" },
        { val: "1", label: "Tuesday" },
        { val: "2", label: "Wednesday" },
        { val: "3", label: "Thursday" },
        { val: "4", label: "Friday" },
        { val: "5", label: "Saturday" },
        { val: "6", label: "Sunday" },
    ];
    
    const daysOfMonth = Array.from({ length: 31 }, (_, i) => (i + 1).toString());

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
                message: "Please select Time, Timezone, and Type.",
                isError: true,
            });
            return;
        }

        const payload: any = {
            hour: parseInt(hour, 10),
            minute: parseInt(minute, 10),
            timezone,
            schedule_type: scheduleType,
            frequency,
        };
        
        if (frequency === "weekly") {
            payload.day_of_week = parseInt(dayOfWeek, 10);
        } else if (frequency === "monthly") {
            payload.day_of_month = parseInt(dayOfMonth, 10);
        }

        try {
            await api("/schedules", "POST", payload);
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

    function deleteSchedule(id: number) {
        popup.set({
            visible: true,
            title: "Confirm Deletion",
            message: "Delete this schedule?",
            isError: false,
            type: "confirm",
            onConfirm: async () => {
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
            },
            onCancel: () => {},
        });
    }

    function formatCron(schedule: any) {
        const cron = schedule.cron_expression;
        if (!cron) return "Unknown";
        
        let localTimeStr = "";
        let dayShift = 0;
        
        if (schedule.time) {
            const date = new Date(schedule.time);
            localTimeStr = date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
        }

        const parts = cron.split(" ");
        if (parts.length < 5) return cron;

        const min = parts[1].padStart(2, "0");
        const hour = parts[2].padStart(2, "0");
        const dom = parts[3];
        const dow = parts[5];
        
        const displayTime = localTimeStr || `${hour}:${min}`;
        
        if (dom === "*" && dow === "*") {
            return `Daily at ${displayTime}`;
        } else if (dom === "*" && dow !== "*") {
             const days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
             let d = parseInt(dow, 10);
             
             if (!isNaN(d)) {
                 d = d + dayShift;
                 if (d < 0) d += 7;
                 d = d % 7;
             }
             
             const dayName = !isNaN(d) && days[d] ? days[d] : dow;
             return `Weekly on ${dayName} at ${displayTime}`;
        } else if (dom !== "*" && dow === "*") {
             return `Monthly on day ${dom} at ${displayTime}`;
        }
        return cron;
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
                    <span class="schedule-time">{formatCron(schedule)}</span>
                    <span class="schedule-type-badge">{schedule.schedule_type || 'rss'}</span>
                </div>
                <button
                    on:click={() => deleteSchedule(schedule.id)}
                    class="delete-btn">×</button
                >
            </li>
        {/each}
    </ul>
    <form on:submit|preventDefault={addSchedule} id="add-schedule-form" class="modern-form">
        <div class="form-grid">
            <div class="input-group frequency-group">
                <select bind:value={frequency} class="modern-select" aria-label="Frequency">
                    <option value="daily">Daily</option>
                    <option value="weekly">Weekly</option>
                    <option value="monthly">Monthly</option>
                </select>
                {#if frequency === "weekly"}
                    <select bind:value={dayOfWeek} class="modern-select" aria-label="Day of Week">
                        {#each daysOfWeek as d}
                            <option value={d.val}>{d.label}</option>
                        {/each}
                    </select>
                {/if}
                {#if frequency === "monthly"}
                    <select bind:value={dayOfMonth} class="modern-select" aria-label="Day of Month">
                        {#each daysOfMonth as d}
                            <option value={d}>{d}</option>
                        {/each}
                    </select>
                {/if}
            </div>

            <div class="input-group time-group">
                <select bind:value={hour} required class="modern-select time-select">
                    <option value="" disabled selected>HH</option>
                    {#each hours as h}
                        <option value={h}>{h}</option>
                    {/each}
                </select>
                <span class="time-separator">:</span>
                <select bind:value={minute} required class="modern-select time-select">
                    <option value="" disabled selected>MM</option>
                    {#each minutes as m}
                        <option value={m}>{m}</option>
                    {/each}
                </select>
            </div>

            <select bind:value={timezone} id="timezone-select" required class="modern-select timezone-select">
                {#each timezones as tz}
                    <option value={tz}>{tz}</option>
                {/each}
            </select>

            <select bind:value={scheduleType} required class="modern-select type-select">
                <option value="rss">RSS Generator</option>
                <option value="read_it_later">Read It Later</option>
            </select>
            
            <button type="submit" class="add-btn-modern">Add Schedule</button>
        </div>
    </form>
</section>
