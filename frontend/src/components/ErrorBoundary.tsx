import { Component, ErrorInfo, ReactNode } from 'react';
import { Button, EmptyState, Icon } from '@canonical/react-components';
import { S } from '../strings/catalogue';

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  message: string;
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { hasError: false, message: '' };

  static getDerivedStateFromError(error: unknown): State {
    return {
      hasError: true,
      message: error instanceof Error ? error.message : S.errorBoundary.unknownError,
    };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('Uncaught error:', error, info.componentStack);
  }

  render() {
    if (this.state.hasError) {
      return (
        <EmptyState title={S.errorBoundary.title} image={<Icon name="error" />}>
          <p>{this.state.message}</p>
          <Button appearance="positive" onClick={() => this.setState({ hasError: false })}>
            {S.errorBoundary.dismiss}
          </Button>
        </EmptyState>
      );
    }

    return this.props.children;
  }
}
