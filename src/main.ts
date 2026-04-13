/// <reference types="svelte" />
import "./app.css";
import { mount } from "svelte";
// @ts-expect-error - No idea why it can't find the svelte types here, but it works fine
import App from "./routes/+page.svelte";

const appElement = document.getElementById("app");
if (!appElement) throw new Error("App element not found");

const app = mount(App, {
	target: appElement,
});

export default app;
