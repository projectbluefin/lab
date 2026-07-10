//#region src/lib/json-script.ts
function serializeJsonScript(value) {
	return JSON.stringify(value).replace(/</g, "\\u003c");
}
//#endregion
export { serializeJsonScript as t };
