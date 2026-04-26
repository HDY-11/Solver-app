import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { info } from '@tauri-apps/plugin-log';
import { useBarContent } from '../components/BarContext';

interface ViewsPageProps {
    display: boolean;
}

const ViewsPage: React.FC<ViewsPageProps> = ({display}) => {
    const [cliContent, setCliContent] = useState<string>('');

    return (
        <div className='page-container' style={{ padding: 0, overflow: 'hidden', display: display ? undefined : 'none' }}>
            <div className='View'>
                <div className='min-content'>1</div>
                <div className='mid-content'>2</div>
                <div className='big-content'>3</div>
            </div>
        </div>
    )
}

export default ViewsPage;