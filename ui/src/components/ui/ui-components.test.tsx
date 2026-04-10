import { fireEvent, screen } from "@testing-library/react";
import { useForm } from "react-hook-form";
import { describe, expect, it } from "vitest";

import { renderWithProviders } from "@/test/render";
import {
  Alert,
  AlertDescription,
  AlertTitle,
  Avatar,
  AvatarFallback,
  Badge,
  Button,
  Calendar,
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  Form,
  FormControl,
  FormDescription,
  FormField,
  FormItem,
  FormLabel,
  Input,
  Label,
  Popover,
  PopoverContent,
  PopoverTrigger,
  Progress,
  ScrollArea,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Separator,
  Sheet,
  SheetContent,
  SheetDescription,
  SheetTitle,
  Skeleton,
  Switch,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui";

function FormExample() {
  const form = useForm({
    defaultValues: {
      name: "",
    },
  });

  return (
    <Form {...form}>
      <form>
        <FormField
          control={form.control}
          name="name"
          render={({ field }) => (
            <FormItem>
              <FormLabel>Name</FormLabel>
              <FormControl>
                <Input aria-label="Name" {...field} />
              </FormControl>
              <FormDescription>Helpful description</FormDescription>
            </FormItem>
          )}
        />
      </form>
    </Form>
  );
}

describe("ui component smoke tests", () => {
  it("renders alert", () => {
    renderWithProviders(
      <Alert>
        <AlertTitle>Heads up</AlertTitle>
        <AlertDescription>Alert description</AlertDescription>
      </Alert>,
    );

    expect(screen.getByRole("alert")).toHaveTextContent("Heads up");
  });

  it("renders avatar", () => {
    renderWithProviders(
      <Avatar>
        <AvatarFallback>AB</AvatarFallback>
      </Avatar>,
    );

    expect(screen.getByText("AB")).toBeInTheDocument();
  });

  it("renders badge", () => {
    renderWithProviders(<Badge variant="warning">Warning</Badge>);

    expect(screen.getByText("Warning")).toBeInTheDocument();
  });

  it("renders button", () => {
    renderWithProviders(<Button>Run action</Button>);

    expect(screen.getByRole("button", { name: "Run action" })).toBeInTheDocument();
  });

  it("renders calendar", () => {
    renderWithProviders(<Calendar month={new Date(2026, 3, 1)} selected={new Date(2026, 3, 10)} />);

    expect(screen.getByRole("grid")).toBeInTheDocument();
  });

  it("renders card", () => {
    renderWithProviders(
      <Card>
        <CardHeader>
          <CardTitle>Card title</CardTitle>
          <CardDescription>Card description</CardDescription>
        </CardHeader>
        <CardContent>Body</CardContent>
        <CardFooter>Footer</CardFooter>
      </Card>,
    );

    expect(screen.getByText("Card title")).toBeInTheDocument();
  });

  it("opens context menu content", () => {
    renderWithProviders(
      <ContextMenu>
        <ContextMenuTrigger>Right click here</ContextMenuTrigger>
        <ContextMenuContent>
          <ContextMenuItem>Rename</ContextMenuItem>
        </ContextMenuContent>
      </ContextMenu>,
    );

    fireEvent.contextMenu(screen.getByText("Right click here"));

    expect(screen.getByText("Rename")).toBeInTheDocument();
  });

  it("renders dialog content", () => {
    renderWithProviders(
      <Dialog open>
        <DialogContent>
          <DialogTitle>Dialog title</DialogTitle>
          <DialogDescription>Dialog description</DialogDescription>
        </DialogContent>
      </Dialog>,
    );

    expect(screen.getByText("Dialog title")).toBeInTheDocument();
  });

  it("renders dropdown menu content", () => {
    renderWithProviders(
      <DropdownMenu open>
        <DropdownMenuTrigger asChild>
          <button type="button">Open menu</button>
        </DropdownMenuTrigger>
        <DropdownMenuContent>
          <DropdownMenuItem>Rename</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>,
    );

    expect(screen.getByText("Rename")).toBeInTheDocument();
  });

  it("renders form field helpers", () => {
    renderWithProviders(<FormExample />);

    expect(screen.getByLabelText("Name")).toBeInTheDocument();
    expect(screen.getByText("Helpful description")).toBeInTheDocument();
  });

  it("renders standalone input", () => {
    renderWithProviders(<Input aria-label="Standalone input" />);

    expect(screen.getByLabelText("Standalone input")).toBeInTheDocument();
  });

  it("renders label association", () => {
    renderWithProviders(
      <div>
        <Label htmlFor="field">Custom label</Label>
        <input id="field" />
      </div>,
    );

    expect(screen.getByLabelText("Custom label")).toBeInTheDocument();
  });

  it("renders popover content", () => {
    renderWithProviders(
      <Popover open>
        <PopoverTrigger asChild>
          <button type="button">Open popover</button>
        </PopoverTrigger>
        <PopoverContent>Popover body</PopoverContent>
      </Popover>,
    );

    expect(screen.getByText("Popover body")).toBeInTheDocument();
  });

  it("renders progressbar", () => {
    renderWithProviders(<Progress value={66} />);

    expect(screen.getByRole("progressbar")).toBeInTheDocument();
  });

  it("renders scroll area", () => {
    renderWithProviders(
      <ScrollArea className="h-16 w-32">
        <div>Scrollable content</div>
      </ScrollArea>,
    );

    expect(screen.getByText("Scrollable content")).toBeInTheDocument();
  });

  it("renders select content", () => {
    renderWithProviders(
      <Select defaultValue="alpha" open>
        <SelectTrigger aria-label="Status">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="alpha">Alpha</SelectItem>
          <SelectItem value="beta">Beta</SelectItem>
        </SelectContent>
      </Select>,
    );

    expect(screen.getByText("Beta")).toBeInTheDocument();
  });

  it("renders separator", () => {
    renderWithProviders(<Separator decorative={false} />);

    expect(screen.getByRole("separator")).toBeInTheDocument();
  });

  it("renders sheet content", () => {
    renderWithProviders(
      <Sheet open>
        <SheetContent>
          <SheetTitle>Sheet title</SheetTitle>
          <SheetDescription>Sheet description</SheetDescription>
        </SheetContent>
      </Sheet>,
    );

    expect(screen.getByText("Sheet title")).toBeInTheDocument();
  });

  it("renders skeleton", () => {
    renderWithProviders(<Skeleton data-testid="skeleton" className="h-4 w-16" />);

    expect(screen.getByTestId("skeleton")).toBeInTheDocument();
  });

  it("renders switch", () => {
    renderWithProviders(<Switch aria-label="Enabled" checked />);

    expect(screen.getByRole("switch", { name: "Enabled" })).toBeInTheDocument();
  });

  it("renders table", () => {
    renderWithProviders(
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Name</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow>
            <TableCell>Alpha</TableCell>
          </TableRow>
        </TableBody>
      </Table>,
    );

    expect(screen.getByRole("table")).toBeInTheDocument();
  });

  it("renders tabs", () => {
    renderWithProviders(
      <Tabs defaultValue="overview">
        <TabsList>
          <TabsTrigger value="overview">Overview</TabsTrigger>
        </TabsList>
        <TabsContent value="overview">Overview content</TabsContent>
      </Tabs>,
    );

    expect(screen.getByText("Overview content")).toBeInTheDocument();
  });

  it("renders tooltip content", () => {
    renderWithProviders(
      <Tooltip open>
        <TooltipTrigger asChild>
          <button type="button">Hover target</button>
        </TooltipTrigger>
        <TooltipContent>Tooltip body</TooltipContent>
      </Tooltip>,
    );

    expect(screen.getByRole("tooltip")).toHaveTextContent("Tooltip body");
  });
});
