<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { browser } from '$app/environment';

	interface RangeData {
		time: number;
		tag_id: number;
		anchor: string;
		distance: number;
		raw: number;
		rssi: number;
		fp_rssi: number;
		round_time: number;
		reply_time: number;
		clock_offset: number;
	}

	interface ErrorData {
		time: number;
        lastdistance: number;
		message: string;
	}

	let chartContainer: HTMLDivElement;
	let chart: any = null;
	let LineSeries: any = null;
	let rangeData = $state<RangeData[]>([]);
	let errorData = $state<ErrorData[]>([]);

	// Series references (reactive so effects can track them)
	let distanceSeries = $state<any>(null);
	let rawSeries = $state<any>(null);
	let rssiSeries = $state<any>(null);
	let fpRssiSeries = $state<any>(null);
	let errorMarkersSeries = $state<any>(null);
	let minTime: number = 0;

	// Visibility toggles
	let showDistance = $state(true);
	let showRaw = $state(true);
	let showRssi = $state(true);
	let showFpRssi = $state(true);
	let showErrors = $state(true);
	let isLoading = $state(true);
	let loadError = $state<string | null>(null);

	// Series colors
	const colors = {
		distance: '#2962FF',
		raw: '#FF6B00',
		rssi: '#00C853',
		fpRssi: '#AA00FF'
	};

	async function loadData() {
		console.log('loadData: starting fetch');
		isLoading = true;
		loadError = null;
		try {
			// Load ranges data
			const rangesResponse = await fetch('/ranges_1.txt');
			console.log('loadData: ranges response status', rangesResponse.status);
			if (!rangesResponse.ok) {
				throw new Error(`Failed to fetch ranges: ${rangesResponse.status}`);
			}
			const rangesText = await rangesResponse.text();
			console.log('loadData: ranges text length', rangesText.length);
			const parsedRanges = parseRangesData(rangesText);
			console.log('Parsed ranges:', parsedRanges.length, 'samples:', parsedRanges.slice(0, 2));
			rangeData = parsedRanges;

			// Load errors data
			const errorsResponse = await fetch('/errors_1.txt');
			console.log('loadData: errors response status', errorsResponse.status);
			if (!errorsResponse.ok) {
				throw new Error(`Failed to fetch errors: ${errorsResponse.status}`);
			}
			const errorsText = await errorsResponse.text();
			const parsedErrors = parseErrorsData(errorsText);
			console.log('Parsed errors:', parsedErrors.length);
			errorData = parsedErrors;

			isLoading = false;

			// Call setup series after data is loaded
			if (chart && LineSeries) {
				console.log('Calling setupSeries from loadData');
				setupSeries();
			}
		} catch (error) {
			console.error('Failed to load data:', error);
			loadError = error instanceof Error ? error.message : 'Unknown error';
			isLoading = false;
		}
	}

	function parseRangesData(text: string): RangeData[] {
		const lines = text.trim().split('\n');
		const data: RangeData[] = [];

		for (const line of lines) {
			const colonIndex = line.indexOf(':');
			if (colonIndex === -1) continue;

			const timestamp = parseInt(line.substring(0, colonIndex));
			const jsonStr = line.substring(colonIndex + 1).trim();

			try {
				const json = JSON.parse(jsonStr);
				const anchors = json.anchors;

				for (const [anchorName, anchorData] of Object.entries(anchors)) {
					const anchor = anchorData as any;
					data.push({
						time: timestamp,
						tag_id: json.tag_id,
						anchor: anchorName,
						distance: anchor.distance / 100.0,
						raw: anchor.raw / 100.0,
						rssi: anchor.rssi,
						fp_rssi: anchor.fp_rssi,
						round_time: anchor.round_time,
						reply_time: anchor.reply_time,
						clock_offset: anchor.clock_offset
					});
				}
			} catch (e) {
				console.warn('Failed to parse line:', line, e);
			}
		}

		return data.sort((a, b) => a.time - b.time);
	}

	function parseErrorsData(text: string): ErrorData[] {
		const lines = text.trim().split('\n');
		const data: ErrorData[] = [];

		for (const line of lines) {
			const colonIndex = line.indexOf(':');
			if (colonIndex === -1) continue;

			const timestamp = parseInt(line.substring(0, colonIndex));
			const message = line.substring(colonIndex + 1).trim();

			data.push({
				time: timestamp,
                lastdistance: rangeData.findLast(d => d.time <= timestamp)?.distance ?? 0,
				message
			});
		}

		return data.sort((a, b) => a.time - b.time);
	}

	function setupSeries() {
		if (!chart || !LineSeries || rangeData.length === 0) {
			console.log('setupSeries: missing requirements', { chart: !!chart, LineSeries: !!LineSeries, dataLength: rangeData.length });
			return;
		}

		console.log('Setting up series with', rangeData.length, 'data points');

		// Clear existing series
		if (distanceSeries) chart.removeSeries(distanceSeries);
		if (rawSeries) chart.removeSeries(rawSeries);
		if (rssiSeries) chart.removeSeries(rssiSeries);
		if (fpRssiSeries) chart.removeSeries(fpRssiSeries);
		if (errorMarkersSeries) chart.removeSeries(errorMarkersSeries);

		// Get time range for normalization
		minTime = rangeData[0].time;

		// Convert milliseconds to seconds (relative to start)
		const toChartTime = (ms: number) => (ms - minTime) / 1000;

		// Distance series - use 'left' price scale (dots only, no line)
		distanceSeries = chart.addSeries(LineSeries, {
			color: colors.distance,
			lineWidth: 0,
			lineVisible: false,
			title: 'Distance (m)',
			priceScaleId: 'left',
			visible: showDistance,
			pointMarkersVisible: true,
			pointMarkersRadius: 3
		});
		const distanceData = rangeData.map((d) => ({
			time: toChartTime(d.time) as any,
			value: d.distance
		}));
		console.log('Distance data sample:', distanceData.slice(0, 3));
		distanceSeries.setData(distanceData);

		// Raw series - same 'left' scale as distance
		rawSeries = chart.addSeries(LineSeries, {
			color: colors.raw,
			lineWidth: 2,
			title: 'Raw (m)',
			priceScaleId: 'left',
			visible: showRaw
		});
		rawSeries.setData(
			rangeData.map((d) => ({
				time: toChartTime(d.time) as any,
				value: d.raw
			}))
		);

		// RSSI series - use 'right' price scale
		// Create data with consistent 150ms intervals for stable timescale
		const rssiDataMap = new Map<number, number>();
		rangeData.forEach((d) => {
			const t = toChartTime(d.time);
			rssiDataMap.set(t, d.rssi);
		});
		
		// Get time range and create regular intervals
		const startTime = 0;
		const endTime = toChartTime(rangeData[rangeData.length - 1].time);
		const interval = 0.15; // 150 milliseconds in seconds
		
		const rssiRegularData: { time: any; value: number }[] = [];
		let lastValue: number | null = null;
		
		for (let t = startTime; t <= endTime; t += interval) {
			// Round to avoid floating point issues
			const roundedT = Math.round(t * 1000) / 1000;
			
			// Find closest actual data point within this interval
			let closestValue: number | null = null;
			let closestDist = Infinity;
			
			for (const [dataTime, value] of rssiDataMap) {
				const dist = Math.abs(dataTime - roundedT);
				if (dist < interval / 2 && dist < closestDist) {
					closestDist = dist;
					closestValue = value;
				}
			}
			
			if (closestValue !== null) {
				lastValue = closestValue;
			}
			
			// Use the closest value, or the last known value, or skip if no data yet
			if (lastValue !== null) {
				rssiRegularData.push({ time: roundedT as any, value: lastValue });
			}
		}
		
		rssiSeries = chart.addSeries(LineSeries, {
			color: colors.rssi,
			lineWidth: 2,
			title: 'RSSI (dBm)',
			priceScaleId: 'right',
			visible: showRssi
		});
		rssiSeries.setData(rssiRegularData);

		// FP RSSI series - same 'right' scale as RSSI
		fpRssiSeries = chart.addSeries(LineSeries, {
			color: colors.fpRssi,
			lineWidth: 2,
			title: 'FP RSSI (dBm)',
			priceScaleId: 'right',
			visible: showFpRssi
		});
		fpRssiSeries.setData(
			rangeData.map((d) => ({
				time: toChartTime(d.time) as any,
				value: d.fp_rssi
			}))
		);

		// Create error markers using vertical lines spanning the chart
		if (errorData.length > 0) {
			// Get the distance range for error line placement
			const maxDistance = Math.max(...rangeData.map(d => d.distance));
			const minDistance = Math.min(...rangeData.map(d => d.distance));
			
			// Create error lines as pairs of points (vertical lines)
			// We'll use multiple short line segments to create vertical markers
			const errorSeriesData: {time: any, value: number}[] = [];
			
			for (const e of errorData) {
				const t = toChartTime(e.time);
				// Add points for a vertical line at this time
				errorSeriesData.push({ time: t as any, value: e.lastdistance });
			}
			
			// Sort by time
			errorSeriesData.sort((a, b) => a.time - b.time);
			
			// Remove duplicates (same time)
			const uniqueErrorData = errorSeriesData.filter((item, index, arr) => 
				index === 0 || item.time !== arr[index - 1].time
			);
			
			errorMarkersSeries = chart.addSeries(LineSeries, {
				color: '#FF0000',
				lineWidth: 0,
				lineVisible: false,
				priceScaleId: 'left',
				visible: showErrors,
				lastValueVisible: false,
				priceLineVisible: false,
				crosshairMarkerVisible: true,
				crosshairMarkerRadius: 6,
				pointMarkersVisible: true,
				pointMarkersRadius: 4
			});

			errorMarkersSeries.setData(uniqueErrorData);
			console.log('Error markers added:', uniqueErrorData.length);
		}

		// Fit content
		chart.timeScale().fitContent();
		console.log('Series setup complete, fitContent called');
	}

	function handleResize() {
		if (chart && chartContainer) {
			chart.applyOptions({
				width: chartContainer.clientWidth,
				height: chartContainer.clientHeight
			});
		}
	}

	onMount(async () => {
		if (!browser) return;

		const lc = await import('lightweight-charts');
		const { createChart, ColorType } = lc;
		LineSeries = lc.LineSeries;

		chart = createChart(chartContainer, {
			width: chartContainer.clientWidth,
			height: 500,
			layout: {
				background: { type: ColorType.Solid, color: '#1a1a2e' },
				textColor: '#d1d4dc'
			},
			grid: {
				vertLines: { color: '#2B2B43' },
				horzLines: { color: '#2B2B43' }
			},
			crosshair: {
				mode: 1
			},
			rightPriceScale: {
				visible: true,
				borderColor: colors.rssi
			},
			leftPriceScale: {
				visible: true,
				borderColor: colors.distance
			},
			timeScale: {
				borderColor: '#485c7b',
				timeVisible: true,
				secondsVisible: true,
				tickMarkFormatter: (time: number) => {
					const totalSeconds = time;
					const minutes = Math.floor(totalSeconds / 60);
					const seconds = (totalSeconds % 60).toFixed(1);
					return `${minutes}:${seconds.padStart(4, '0')}`;
				}
			},
			handleScroll: {
				mouseWheel: true,
				pressedMouseMove: true
			},
			handleScale: {
				mouseWheel: true,
				pinch: true,
				axisPressedMouseMove: true
			}
		});

		loadData();

		window.addEventListener('resize', handleResize);
	});

	onDestroy(() => {
		if (browser) {
			if (chart) {
				chart.remove();
				chart = null;
			}
			window.removeEventListener('resize', handleResize);
		}
	});

	// Setup series when data changes
	$effect(() => {
		if (chart && LineSeries && rangeData.length > 0 && !distanceSeries) {
			console.log('Effect: setting up series because data loaded');
			setupSeries();
		}
	});

	// Reactive updates when toggles change - separate effects for each toggle
	$effect(() => {
		const visible = showDistance;
		const series = distanceSeries;
		if (series) {
			series.applyOptions({ visible });
		}
	});

	$effect(() => {
		const visible = showRaw;
		const series = rawSeries;
		if (series) {
			series.applyOptions({ visible });
		}
	});

	$effect(() => {
		const visible = showRssi;
		const series = rssiSeries;
		if (series) {
			series.applyOptions({ visible });
		}
	});

	$effect(() => {
		const visible = showFpRssi;
		const series = fpRssiSeries;
		if (series) {
			series.applyOptions({ visible });
		}
	});

	// Effect for error markers visibility
	$effect(() => {
		const visible = showErrors;
		const series = errorMarkersSeries;
		if (series) {
			series.applyOptions({ visible });
		}
	});
</script>

<div class="chart-wrapper">
	<div class="controls">
		<h2>UWB Range Measurement Data</h2>
		<div class="toggles">
			<label class="toggle" style="--color: {colors.distance}">
				<input type="checkbox" bind:checked={showDistance} />
				<span class="checkmark"></span>
				Distance
			</label>
			<label class="toggle" style="--color: {colors.raw}">
				<input type="checkbox" bind:checked={showRaw} />
				<span class="checkmark"></span>
				Raw
			</label>
			<label class="toggle" style="--color: {colors.rssi}">
				<input type="checkbox" bind:checked={showRssi} />
				<span class="checkmark"></span>
				RSSI
			</label>
			<label class="toggle" style="--color: {colors.fpRssi}">
				<input type="checkbox" bind:checked={showFpRssi} />
				<span class="checkmark"></span>
				FP RSSI
			</label>
			<label class="toggle" style="--color: #FF0000">
				<input type="checkbox" bind:checked={showErrors} />
				<span class="checkmark"></span>
				Errors
			</label>
		</div>
		<div class="legend">
			<div class="legend-item">
				<span class="legend-color" style="background: {colors.distance}"></span>
				<span>Distance / Raw (m) - Left Scale</span>
			</div>
			<div class="legend-item">
				<span class="legend-color" style="background: {colors.rssi}"></span>
				<span>RSSI / FP RSSI (dBm) - Right Scale</span>
			</div>
		</div>
		<div class="instructions">
			<strong>Navigation:</strong> Scroll to zoom • Click and drag to pan • Double-click to reset
		</div>
	</div>
	<div class="chart-container" bind:this={chartContainer}>
		{#if isLoading}
			<div class="loading">Loading data...</div>
		{:else if loadError}
			<div class="error">Error: {loadError}</div>
		{/if}
	</div>
	<div class="stats">
		<div class="stat">
			<span class="stat-label">Data Points:</span>
			<span class="stat-value">{rangeData.length}</span>
		</div>
		<div class="stat">
			<span class="stat-label">Errors:</span>
			<span class="stat-value">{errorData.length}</span>
		</div>
		{#if rangeData.length > 0}
			<div class="stat">
				<span class="stat-label">Time Range:</span>
				<span class="stat-value">
					{((rangeData[rangeData.length - 1].time - rangeData[0].time) / 1000).toFixed(1)}s
				</span>
			</div>
		{/if}
	</div>
</div>

<style>
	.chart-wrapper {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		padding: 1rem;
		background: #0f0f1a;
		border-radius: 12px;
		max-width: 1400px;
		margin: 0 auto;
	}

	.controls {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	h2 {
		margin: 0;
		color: #fff;
		font-size: 1.5rem;
	}

	.toggles {
		display: flex;
		flex-wrap: wrap;
		gap: 1rem;
	}

	.toggle {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		cursor: pointer;
		color: #d1d4dc;
		font-size: 0.9rem;
		user-select: none;
	}

	.toggle input {
		display: none;
	}

	.checkmark {
		width: 18px;
		height: 18px;
		border: 2px solid var(--color);
		border-radius: 4px;
		position: relative;
		transition: background 0.2s;
	}

	.toggle input:checked + .checkmark {
		background: var(--color);
	}

	.checkmark::after {
		content: '';
		position: absolute;
		left: 5px;
		top: 1px;
		width: 4px;
		height: 9px;
		border: solid #fff;
		border-width: 0 2px 2px 0;
		transform: rotate(45deg);
		opacity: 0;
		transition: opacity 0.2s;
	}

	.toggle input:checked + .checkmark::after {
		opacity: 1;
	}

	.legend {
		display: flex;
		flex-wrap: wrap;
		gap: 1.5rem;
	}

	.legend-item {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		color: #888;
		font-size: 0.85rem;
	}

	.legend-color {
		width: 24px;
		height: 3px;
		border-radius: 2px;
	}

	.instructions {
		color: #666;
		font-size: 0.8rem;
	}

	.chart-container {
		width: 100%;
		height: 500px;
		border-radius: 8px;
		overflow: hidden;
		position: relative;
	}

	.loading,
	.error {
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		color: #d1d4dc;
		font-size: 1.2rem;
		z-index: 10;
	}

	.error {
		color: #ff6b6b;
	}

	.stats {
		display: flex;
		gap: 2rem;
		padding: 0.75rem 1rem;
		background: #1a1a2e;
		border-radius: 8px;
	}

	.stat {
		display: flex;
		gap: 0.5rem;
	}

	.stat-label {
		color: #888;
		font-size: 0.85rem;
	}

	.stat-value {
		color: #fff;
		font-size: 0.85rem;
		font-weight: 600;
	}
</style>
