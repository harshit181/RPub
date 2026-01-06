<script lang="ts">
    import { authHeader, isAuthenticated, isLoginVisible } from "../lib/store";

    let username = "";
    let password = "";
    let error = "";

    async function handleLogin() {
        if (!username || !password) return;

        const creds = "Basic " + btoa(username + ":" + password);

        try {
            const res = await fetch("/auth/check", {
                headers: { Authorization: creds },
            });

            if (res.ok) {
                authHeader.set(creds);
                isAuthenticated.set(true);
                isLoginVisible.set(false);
                error = "";
            } else {
                error = "Invalid credentials";
            }
        } catch (e) {
            error = "Login failed";
        }
    }
</script>

{#if $isLoginVisible}
    <div id="login-overlay">
        <div id="login-box">
            <h2>Login Required</h2>
            <form on:submit|preventDefault={handleLogin} id="login-form">
                <input
                    type="text"
                    bind:value={username}
                    placeholder="Username"
                    required
                />
                <input
                    type="password"
                    bind:value={password}
                    placeholder="Password"
                    required
                />
                <button class="add-btn" type="submit">Login</button>
            </form>
            {#if error}
                <p id="login-error" style="color: var(--danger)">{error}</p>
            {/if}
        </div>
    </div>
{/if}
