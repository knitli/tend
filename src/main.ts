/// <reference types="svelte" />
import "./app.css";
import { mount } from "svelte";
import App from "./routes/+page.svelte";

const appElement = document.getElementById("app");
if (!appElement) throw new Error("App element not found");

const app = mount(App, {
	target: appElement,
});

export default app;
