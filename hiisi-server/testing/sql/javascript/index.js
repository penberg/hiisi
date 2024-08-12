import { createClient } from "@libsql/client";

const client = createClient({
    url: "http://127.0.0.1:8080",
});

await client.execute("SELECT 1");