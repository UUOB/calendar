import { boot } from '../uuob/ui/boot.js';
import Calendar from './Calendar.svelte';

boot({ id: 'calendar', name: 'Calendar', icon: 'calendar', color: 'var(--c-red)', noun: 'events' }, Calendar);
