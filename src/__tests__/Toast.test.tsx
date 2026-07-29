import { describe, it, expect, beforeEach, vi } from 'vitest'
import { render, screen, act } from '@testing-library/react'
import { ToastProvider, useToast } from '../components/Toast'

// Helper: componente que dispara toasts
function ToastTester({ message, type }: { message: string; type?: 'success' | 'error' | 'warning' | 'info' }) {
  const { toast } = useToast()
  return (
    <button onClick={() => toast(message, type)} data-testid="trigger">
      Show Toast
    </button>
  )
}

describe('Toast component', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  it('renderiza el toast al llamar toast()', async () => {
    render(
      <ToastProvider>
        <ToastTester message="Operación exitosa" type="success" />
      </ToastProvider>,
    )

    await act(() => {
      screen.getByTestId('trigger').click()
    })

    expect(screen.getByText('Operación exitosa')).toBeInTheDocument()
  })

  it('muestra el tipo correcto (error → ✕)', async () => {
    render(
      <ToastProvider>
        <ToastTester message="Algo falló" type="error" />
      </ToastProvider>,
    )

    await act(() => {
      screen.getByTestId('trigger').click()
    })

    expect(screen.getByText('✕')).toBeInTheDocument()
    expect(screen.getByText('Algo falló')).toBeInTheDocument()
  })

  it('usa "info" por defecto si no se especifica tipo', async () => {
    render(
      <ToastProvider>
        <ToastTester message="Nota informativa" />
      </ToastProvider>,
    )

    await act(() => {
      screen.getByTestId('trigger').click()
    })

    expect(screen.getByText('ℹ')).toBeInTheDocument()
  })

  it('desaparece después de 4 segundos', async () => {
    render(
      <ToastProvider>
        <ToastTester message="Auto-dismiss" type="warning" />
      </ToastProvider>,
    )

    await act(() => {
      screen.getByTestId('trigger').click()
    })

    expect(screen.getByText('Auto-dismiss')).toBeInTheDocument()

    await act(() => {
      vi.advanceTimersByTime(4000)
    })

    expect(screen.queryByText('Auto-dismiss')).not.toBeInTheDocument()
  })

  it('soporta múltiples toasts simultáneos', async () => {
    render(
      <ToastProvider>
        <ToastTester message="Primero" type="success" />
      </ToastProvider>,
    )

    // Simulamos el contexto directo para múltiples toasts
    function MultiToastTester() {
      const { toast } = useToast()
      return (
        <button
          data-testid="multi"
          onClick={() => {
            toast('Uno', 'info')
            toast('Dos', 'error')
            toast('Tres', 'success')
          }}
        >
          Multi
        </button>
      )
    }

    const { rerender } = render(
      <ToastProvider>
        <MultiToastTester />
      </ToastProvider>,
    )

    await act(() => {
      screen.getByTestId('multi').click()
    })

    expect(screen.getByText('Uno')).toBeInTheDocument()
    expect(screen.getByText('Dos')).toBeInTheDocument()
    expect(screen.getByText('Tres')).toBeInTheDocument()
  })
})
