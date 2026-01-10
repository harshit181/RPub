// Get browser API (works for both Chrome and Firefox)
const browserAPI = typeof browser !== 'undefined' ? browser : chrome;

document.addEventListener('DOMContentLoaded', async () => {
    const form = document.getElementById('options-form');
    const endpointInput = document.getElementById('endpoint');
    const usernameInput = document.getElementById('username');
    const passwordInput = document.getElementById('password');
    const messageEl = document.getElementById('message');
    const testBtn = document.getElementById('test-btn');


    const config = await browserAPI.storage.local.get(['endpoint', 'username', 'password']);
    if (config.endpoint) {
        endpointInput.value = config.endpoint;
    }
    if (config.username) {
        usernameInput.value = config.username;
    }
    if (config.password) {
        passwordInput.value = config.password;
    }


    form.addEventListener('submit', async (e) => {
        e.preventDefault();
        messageEl.textContent = '';
        messageEl.className = 'message';

        const endpoint = endpointInput.value.trim();
        const username = usernameInput.value.trim();
        const password = passwordInput.value;

        if (!endpoint) {
            messageEl.textContent = 'RssPub Host URL is required.';
            messageEl.className = 'message error';
            return;
        }

        try {
            await browserAPI.storage.local.set({
                endpoint: endpoint,
                username: username,
                password: password
            });
            messageEl.textContent = 'Settings saved successfully!';
            messageEl.className = 'message success';
        } catch (error) {
            messageEl.textContent = `Error saving settings: ${error.message}`;
            messageEl.className = 'message error';
        }
    });


    testBtn.addEventListener('click', async () => {
        messageEl.textContent = '';
        messageEl.className = 'message';

        const endpoint = endpointInput.value.trim();
        const username = usernameInput.value.trim();
        const password = passwordInput.value;

        if (!endpoint) {
            messageEl.textContent = 'Please enter a RssPub Host URL first.';
            messageEl.className = 'message error';
            return;
        }

        testBtn.disabled = true;
        testBtn.textContent = 'Testing...';

        try {
            const headers = {
                'Content-Type': 'application/json'
            };

            if (username && password) {
                const credentials = btoa(`${username}:${password}`);
                headers['Authorization'] = `Basic ${credentials}`;
            }


            const baseUrl = endpoint.replace(/\/$/, '');
            const fullUrl = `${baseUrl}/read-it-later`;

            const response = await fetch(fullUrl, {
                method: 'POST',
                headers: headers,
                body: JSON.stringify({ url: 'https://example.com/test' })
            });

            if (response.ok) {
                messageEl.textContent = `Connection successful! Status: ${response.status}`;
                messageEl.className = 'message success';
            } else {
                messageEl.textContent = `Server responded with: ${response.status} ${response.statusText}`;
                messageEl.className = 'message error';
            }
        } catch (error) {
            messageEl.textContent = `Connection failed: ${error.message}`;
            messageEl.className = 'message error';
        } finally {
            testBtn.disabled = false;
            testBtn.textContent = 'Test Connection';
        }
    });
});
