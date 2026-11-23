package survarium.menu.selector
{
   import flash.display.BitmapData;
   import flash.display.DisplayObject;
   import flash.display.Graphics;
   import flash.display.MovieClip;
   import flash.display.Shape;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.EventPhase;
   import flash.events.KeyboardEvent;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.geom.Matrix;
   import flash.geom.Point;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.text.TextFormatAlign;
   import flash.ui.Keyboard;
   import scaleform.clik.controls.Button;
   import scaleform.clik.events.ButtonEvent;
   import survarium.constants.UiSound;

   // The Controls' action selector (the owner, 2026-11-23: "selector should be its own thing", in the
   // game's own style): a window over the menu in the Settings window's skin (its background art, title
   // band, close button, the stock text input, key rows, scroll bar, bottom bar and buttons), opened for
   // one key. It lists every action the player may give a key, as the Settings' own key rows with each
   // action's key now, under its kind's heading (SrpKeysWindow.KINDS: the kind's colour only as a mark);
   // the search field filters them as one types. Up and Down (Page Up, Page Down) move through them,
   // Enter, a click or Assign picks; Unbind leaves the key with none; Esc, Cancel, the close button or a
   // click outside cancels. Over the bottom bar, in amber, what else the pick changes: the action moved
   // from its key, the key taken from another action (the game's conflict rule: one lobby action and one
   // of the match a key).
   //
   // The Key bindings window (SrpKeysWindow) asks it through SrpKeysWindow.selector, which the main
   // menu sets to `select`:
   //    select(request, answer)
   //    request  {key: the binder's key id ("kSPACE", "mouse1"), keyName: its text, actions: the rows
   //             bound to it, rows: every row (root.set_keybindings' objects), kinds: SrpKeysWindow.KINDS}
   //    answer   called once, after the window has closed: answer(actionId) the action picked for the
   //             key, answer(UNBIND) the key to have no action, answer(CANCEL) nothing changes
   // and the window applies it (srp_bind_key). close() takes it away answering nothing (the host closing
   // over it); isOpen says whether it is open.
   public class ActionSelector extends Sprite
   {

      public static const CANCEL:int = -1;

      // the Key bindings window's own (its answer to take the key from its rows)
      public static const UNBIND:int = SrpKeysWindow.UNBIND;

      // the Settings window's width (its background art's)
      public static const WIDTH:Number = 650;

      public static const MAX_HEIGHT:Number = 720;

      // around the window, to the screen's edges at the least
      private static const MARGIN:Number = 24;

      // the Settings' layout: its content's left, its art's title band and foot (its bottom bar at stock
      // y 479.9 of 565, 50 tall), its buttons (120 and 118 wide, 34 tall, at 200.55 and 325.9)
      private static const LEFT:Number = 27;

      private static const ART_TOP:Number = 40;

      private static const ART_BOTTOM:Number = 72;

      private static const BAR:Number = 50;

      private static const ROW_X:Number = 18;

      private static const ROW_WIDTH:Number = WIDTH - ROW_X - 42;

      private static const HEADER_Y:Number = 50;

      private static const SEARCH_Y:Number = 92;

      private static const LIST_Y:Number = 134;

      // the warning's two lines over the bottom bar
      private static const NOTE:Number = 46;

      private static const NAVIGATION:Array = [Keyboard.UP,Keyboard.DOWN,Keyboard.PAGE_UP,Keyboard.PAGE_DOWN,Keyboard.ENTER,Keyboard.ESCAPE,Keyboard.TAB];

      // The exe's texts; null until it answers.
      private var texts:SelectorTexts = null;

      // The Settings window's title, whose look the title takes (MainMenu gives it).
      public var titleStyle:TextField = null;

      // The open window's state; null while closed.
      private var session:Session = null;

      private var backdrop:Sprite;

      public var panel:Sprite;

      public var input:MovieClip;

      public var search:TextField;

      private var placeholder:TextField;

      public var header:MovieClip;

      public var list:Sprite;

      private var listMask:Shape;

      private var listHeight:Number;

      public var scrollbar:MovieClip;

      private var empty:TextField;

      public var warning:TextField;

      public var assignButton:Button;

      public var unbindButton:Button;

      public var cancelButton:Button;

      public var closeButton:Button;

      public function ActionSelector()
      {
         super();
         visible = false;
      }

      public function get isOpen() : Boolean
      {
         return this.session != null;
      }

      // The rows on screen, top to bottom: GroupHeading and ActionRow.
      public function get rows() : Array
      {
         return this.session == null ? [] : this.session.rows.concat();
      }

      // The row the keys and the mouse are on (null: none, nothing matches).
      public function get highlighted() : SelectorRow
      {
         return this.session == null ? null : this.session.highlighted;
      }

      public function get scroll() : Number
      {
         return this.session == null ? 0 : this.session.scroll;
      }

      public function get viewHeight() : Number
      {
         return this.listHeight;
      }

      // root.srp_controls_texts: SelectorTexts' order.
      public function setTexts(values:Array) : void
      {
         this.texts = new SelectorTexts(values);
         if(this.session != null)
         {
            this.build();
         }
      }

      public function select(request:Object, answer:Function) : void
      {
         if(this.session != null)
         {
            this.close();
         }
         if(this.texts == null)
         {
            // the exe answers before the call returns (root.srp_controls_texts)
            SrpExe.call("srp_controls_texts",null);
         }
         this.session = new Session(String(request.keyName),BoundAction.parse(request.rows,request.actions),answer);
         visible = true;
         this.build();
         stage.addEventListener(KeyboardEvent.KEY_DOWN,this.onKey,true,1000);
         stage.addEventListener(KeyboardEvent.KEY_UP,this.onKey,true,1000);
         stage.addEventListener(KeyboardEvent.KEY_DOWN,this.onKey,false,1000);
         stage.addEventListener(KeyboardEvent.KEY_UP,this.onKey,false,1000);
         stage.addEventListener(Event.RESIZE,this.onResize);
         addEventListener(KeyboardEvent.KEY_DOWN,this.keep);
         addEventListener(KeyboardEvent.KEY_UP,this.keep);
         stage.focus = this.search;
      }

      public function close() : void
      {
         if(this.session == null)
         {
            return;
         }
         this.session = null;
         stage.removeEventListener(KeyboardEvent.KEY_DOWN,this.onKey,true);
         stage.removeEventListener(KeyboardEvent.KEY_UP,this.onKey,true);
         stage.removeEventListener(KeyboardEvent.KEY_DOWN,this.onKey,false);
         stage.removeEventListener(KeyboardEvent.KEY_UP,this.onKey,false);
         stage.removeEventListener(Event.RESIZE,this.onResize);
         removeEventListener(KeyboardEvent.KEY_DOWN,this.keep);
         removeEventListener(KeyboardEvent.KEY_UP,this.keep);
         if(stage.focus == this.search)
         {
            stage.focus = null;
         }
         while(numChildren > 0)
         {
            removeChildAt(0);
         }
         // the next open starts with an empty search
         this.search = null;
         visible = false;
      }

      // Up and Down: the next action that way, `steps` of them.
      public function move(steps:int) : void
      {
         if(this.session == null)
         {
            return;
         }
         var choices:Array = this.session.choices();
         if(choices.length == 0)
         {
            return;
         }
         var at:int = choices.indexOf(this.session.highlighted);
         var to:int = at == -1 ? (steps > 0 ? 0 : choices.length - 1) : Math.max(0,Math.min(choices.length - 1,at + steps));
         this.light(choices[to]);
         this.reveal(choices[to]);
      }

      // Enter, a click or Assign: the action of `row`.
      public function choose(row:SelectorRow) : void
      {
         if(this.session != null && row is ActionRow)
         {
            this.finish(int(ActionRow(row).action.id));
         }
      }

      public function unbind() : void
      {
         if(this.session != null && this.session.onKey().length > 0)
         {
            this.finish(UNBIND);
         }
      }

      public function cancel() : void
      {
         if(this.session != null)
         {
            this.finish(CANCEL);
         }
      }

      private function finish(answer:int) : void
      {
         var session:Session = this.session;
         if(answer != CANCEL)
         {
            this.sound(UiSound.CLICK);
         }
         this.close();
         session.answer(answer);
      }

      // ---- the window

      private function build() : void
      {
         var query:String = this.search != null ? this.search.text : "";
         while(numChildren > 0)
         {
            removeChildAt(0);
         }
         var t:SelectorTexts = this.texts != null ? this.texts : new SelectorTexts(null);
         var from:Point = globalToLocal(new Point(0,0));
         var to:Point = globalToLocal(new Point(stage.stageWidth,stage.stageHeight));
         var height:Number = Math.round(Math.max(420,Math.min(MAX_HEIGHT,to.y - from.y - 2 * MARGIN)));
         // the menu dimmed under it, as under the game's messages
         this.backdrop = new Sprite();
         this.backdrop.graphics.beginFill(0,0.6);
         this.backdrop.graphics.drawRect(from.x,from.y,to.x - from.x,to.y - from.y);
         this.backdrop.graphics.endFill();
         this.backdrop.addEventListener(MouseEvent.CLICK,this.onBackdrop);
         addChild(this.backdrop);
         this.panel = new Sprite();
         this.panel.name = "_srp_action_selector";
         this.panel.x = Math.round((from.x + to.x - WIDTH) / 2);
         this.panel.y = Math.round((from.y + to.y - height) / 2);
         this.panel.addEventListener(MouseEvent.CLICK,this.onPanelClick);
         addChild(this.panel);
         this.drawArt(height);
         this.buildTitle(t);
         this.buildHeader(t);
         this.buildSearch(t,query);
         this.listHeight = height - ART_BOTTOM - NOTE - LIST_Y;
         this.list = new Sprite();
         this.list.x = ROW_X;
         this.list.y = LIST_Y;
         this.listMask = new Shape();
         this.listMask.graphics.beginFill(0);
         this.listMask.graphics.drawRect(0,LIST_Y,WIDTH,this.listHeight);
         this.listMask.graphics.endFill();
         this.panel.addChild(this.listMask);
         this.panel.addChild(this.list);
         this.list.mask = this.listMask;
         this.list.addEventListener(MouseEvent.MOUSE_MOVE,this.onRowMove);
         this.list.addEventListener(MouseEvent.CLICK,this.onRowClick);
         this.panel.addEventListener(MouseEvent.MOUSE_WHEEL,this.onWheel);
         this.empty = Paint.text(15,Paint.DIM,this.labelFilters());
         this.empty.text = t.noMatch;
         this.empty.x = Math.round((WIDTH - this.empty.width) / 2);
         this.empty.y = LIST_Y + 30;
         this.panel.addChild(this.empty);
         this.scrollbar = new SrvScrollBar();
         this.scrollbar.alpha = 1;
         this.scrollbar.x = WIDTH - 22;
         this.scrollbar.y = LIST_Y;
         this.scrollbar.height = this.listHeight;
         this.scrollbar.addEventListener(Event.SCROLL,this.onScrollBar);
         this.panel.addChild(this.scrollbar);
         this.warning = new TextField();
         this.warning.name = "_warning";
         this.warning.embedFonts = true;
         this.warning.selectable = false;
         this.warning.mouseEnabled = false;
         this.warning.multiline = true;
         this.warning.wordWrap = true;
         this.warning.autoSize = TextFieldAutoSize.LEFT;
         this.warning.defaultTextFormat = Paint.format(15,Paint.WARNING);
         this.warning.filters = this.labelFilters();
         this.warning.x = LEFT;
         this.warning.y = height - ART_BOTTOM - NOTE + 4;
         this.warning.width = WIDTH - 2 * LEFT;
         this.panel.addChild(this.warning);
         this.buildButtons(t,height);
         this.fill(t);
      }

      // The Settings window's background art: its title band and foot as they are, its middle stretched
      // to the height; the bottom bar over it as the Settings' (black 0.7, the hint lines' pattern).
      private function drawArt(height:Number) : void
      {
         var art:BitmapData = new SrpSettingsGround();
         var g:Graphics = this.panel.graphics;
         var m:Matrix = new Matrix();
         g.beginBitmapFill(art,m,false,true);
         g.drawRect(0,0,WIDTH,ART_TOP);
         g.endFill();
         var middle:Number = height - ART_TOP - ART_BOTTOM;
         var scale:Number = middle / (art.height - ART_TOP - ART_BOTTOM);
         m = new Matrix(1,0,0,scale,0,ART_TOP - ART_TOP * scale);
         g.beginBitmapFill(art,m,false,true);
         g.drawRect(0,ART_TOP,WIDTH,middle);
         g.endFill();
         m = new Matrix(1,0,0,1,0,height - art.height);
         g.beginBitmapFill(art,m,false,true);
         g.drawRect(0,height - ART_BOTTOM,WIDTH,ART_BOTTOM);
         g.endFill();
         var bar:Sprite = new Sprite();
         bar.mouseEnabled = false;
         bar.x = 3;
         bar.y = height - ART_BOTTOM;
         bar.graphics.beginFill(0,0.7);
         bar.graphics.drawRect(0,0,WIDTH - 7,BAR);
         bar.graphics.endFill();
         bar.graphics.beginBitmapFill(new HintLinePattern());
         bar.graphics.drawRect(0,0,WIDTH - 7,BAR);
         bar.graphics.endFill();
         this.panel.addChild(bar);
      }

      private function buildTitle(t:SelectorTexts) : void
      {
         var title:TextField = new TextField();
         title.name = "_title";
         title.embedFonts = true;
         title.selectable = false;
         title.mouseEnabled = false;
         var format:TextFormat = this.titleStyle != null ? this.titleStyle.getTextFormat() : Paint.format(17,Paint.TITLE);
         format.align = TextFormatAlign.CENTER;
         title.defaultTextFormat = format;
         if(this.titleStyle != null)
         {
            title.filters = this.titleStyle.filters;
         }
         title.x = 50.75;
         title.y = 5.75;
         title.width = 548.5;
         title.height = 29.65;
         title.text = t.title;
         this.panel.addChild(title);
         this.closeButton = new ButtonCloseWindow2();
         this.closeButton.name = "_close";
         this.closeButton.alpha = 1;
         this.closeButton.width = this.closeButton.height = 31;
         this.closeButton.x = 615.5;
         this.closeButton.y = 5.75;
         this.closeButton.addEventListener(ButtonEvent.CLICK,function(e:ButtonEvent):void
         {
            cancel();
         });
         this.panel.addChild(this.closeButton);
      }

      // The key being edited as a stock key row: its key box, then the action or actions on it now
      // (or "Free key").
      private function buildHeader(t:SelectorTexts) : void
      {
         var now:Array = this.session.onKey();
         var labels:Array = now.map(function(action:BoundAction, ... rest):String
         {
            return action.label;
         });
         var text:String = now.length > 0 ? labels.join(", ") : t.free;
         this.header = new SrvBindKeyListRender();
         this.header.name = "_key";
         this.header.mouseEnabled = this.header.mouseChildren = false;
         this.header.alpha = 1;
         this.header.x = ROW_X + 12;
         this.header.y = HEADER_Y;
         this.header.data = {label:text,key:this.session.keyName,type:1};
         this.panel.addChild(this.header);
         ActionRow.widen(this.header);
         this.header.textField.width = WIDTH - LEFT - this.header.x - this.header.textField.x;
         Paint.fit(this.header.textField,text,this.header.textField.width);
         if(now.length == 0)
         {
            this.header.textField.textColor = Paint.DIM;
         }
         for(var i:int = 0; i < now.length; i++)
         {
            // each action's kind as its mark, left of the key box
            this.panel.graphics.beginFill(BoundAction(now[i]).colour,0.85);
            this.panel.graphics.drawRect(ROW_X + 5,HEADER_Y + 6 + i * 9,3,7);
            this.panel.graphics.endFill();
         }
      }

      private function buildSearch(t:SelectorTexts, query:String) : void
      {
         this.input = new TextInputFriendName();
         this.input.name = "_search";
         this.input.alpha = 1;
         this.input.x = LEFT;
         this.input.y = SEARCH_Y;
         this.panel.addChild(this.input);
         this.input.validateNow();
         this.search = this.input.textField;
         this.search.maxChars = 40;
         this.input.text = query;
         this.search.setSelection(query.length,query.length);
         this.placeholder = Paint.text(15,Paint.DIM);
         Paint.fit(this.placeholder,t.search,this.search.width - 4);
         this.placeholder.x = this.input.x + this.search.x + 2;
         this.placeholder.y = this.input.y + this.search.y + Math.round((this.search.height - this.placeholder.height) / 2);
         this.panel.addChild(this.placeholder);
         this.search.addEventListener(Event.CHANGE,this.onSearch);
         this.placeholder.visible = query.length == 0;
      }

      // The bottom bar's buttons (the Settings' Apply and Exit's, SvButton2, at their sizes): Unbind at
      // the left (while the key has an action, as wide as its words need), Assign and Cancel at the right.
      private function buildButtons(t:SelectorTexts, height:Number) : void
      {
         var y:Number = height - ART_BOTTOM + 11;
         this.unbindButton = this.button("_unbind",t.unbind,LEFT,y,140);
         this.unbindButton.enabled = this.session.onKey().length > 0;
         this.unbindButton.addEventListener(ButtonEvent.CLICK,function(e:ButtonEvent):void
         {
            unbind();
         });
         this.assignButton = this.button("_assign",t.assign,WIDTH - LEFT - 118 - 6 - 120,y,120);
         this.assignButton.addEventListener(ButtonEvent.CLICK,function(e:ButtonEvent):void
         {
            choose(highlighted);
         });
         this.cancelButton = this.button("_cancel",t.cancel,WIDTH - LEFT - 118,y,118);
         this.cancelButton.addEventListener(ButtonEvent.CLICK,function(e:ButtonEvent):void
         {
            cancel();
         });
      }

      private function button(name:String, label:String, x:Number, y:Number, width:Number) : Button
      {
         var b:Button = new SvButton2();
         b.name = name;
         b.autoSize = "none";
         b.toggle = false;
         // Scaleform leaves a script-created control fully transparent
         b.alpha = 1;
         b.label = label;
         b.x = x;
         b.y = y;
         b.width = width;
         b.height = 34;
         this.panel.addChild(b);
         b.validateNow();
         // wider for its words (the Vulkan options' button's rule)
         var room:Number = b.textField != null ? b.textField.width * b.scaleX : width;
         var needed:Number = b.textField != null ? b.textField.textWidth + 8 : 0;
         if(needed > room)
         {
            b.width = width + needed - room;
            b.validateNow();
         }
         return b;
      }

      private function labelFilters() : Array
      {
         return this.header != null && this.header.textField != null ? this.header.textField.filters : null;
      }

      // The rows for the search field's text, the highlight kept where it can be.
      private function fill(t:SelectorTexts) : void
      {
         var keep:SelectorRow = this.session.highlighted;
         var keptAction:BoundAction = keep is ActionRow ? ActionRow(keep).action : null;
         while(this.list.numChildren > 0)
         {
            this.list.removeChildAt(0);
         }
         var rows:Array = [];
         var words:Array = this.search.text.toLowerCase().split(/\s+/).filter(function(w:String, ... rest):Boolean
         {
            return w.length > 0;
         });
         var now:Array = this.session.onKey();
         var filters:Array = this.labelFilters();
         for each(var kind:int in this.session.kinds())
         {
            var heading:GroupHeading = null;
            for each(var action:BoundAction in this.session.ofKind(kind))
            {
               if(!matches(action.label + " " + (kind == -1 ? "" : t.groups[kind]),action.key,words))
               {
                  continue;
               }
               if(heading == null && kind != -1)
               {
                  heading = new GroupHeading(kind,t,ROW_WIDTH,filters);
                  rows.push(heading);
               }
               rows.push(new ActionRow(action,t,ROW_WIDTH));
            }
         }
         var y:Number = 0;
         for each(var row:SelectorRow in rows)
         {
            row.y = y;
            y += row.rowHeight;
            this.list.addChild(row);
         }
         this.session.rows = rows;
         this.session.highlighted = null;
         this.empty.visible = rows.length == 0;
         // the same action again, else the key's action, else the first action
         var light:SelectorRow = null;
         for each(row in rows)
         {
            if(row is ActionRow && ActionRow(row).action == keptAction)
            {
               light = row;
            }
         }
         if(light == null && keep == null && now.length > 0)
         {
            for each(row in rows)
            {
               if(light == null && row is ActionRow && ActionRow(row).action == now[0])
               {
                  light = row;
               }
            }
         }
         for each(row in rows)
         {
            if(light == null && row is ActionRow)
            {
               light = row;
            }
         }
         this.session.scroll = Math.min(this.session.scroll,this.maxScroll());
         this.light(light);
         this.reveal(light,true);
      }

      private static function matches(text:String, key:String, words:Array) : Boolean
      {
         // the whole search a key's name: that key's actions
         if(key != null && words.length > 0 && key.toLowerCase() == words.join(" "))
         {
            return true;
         }
         var lower:String = text.toLowerCase();
         for each(var word:String in words)
         {
            if(lower.indexOf(word) == -1)
            {
               return false;
            }
         }
         return true;
      }

      private function light(row:SelectorRow) : void
      {
         if(this.session.highlighted != null)
         {
            this.session.highlighted.highlight(false);
         }
         this.session.highlighted = row;
         if(row != null)
         {
            row.highlight(true);
         }
         this.assignButton.enabled = row is ActionRow;
         this.note();
      }

      // Over the bottom bar: what else the highlighted pick changes.
      private function note() : void
      {
         var t:SelectorTexts = this.texts != null ? this.texts : new SelectorTexts(null);
         var notes:Array = [];
         var row:ActionRow = this.session.highlighted as ActionRow;
         if(row != null && !row.action.onKey)
         {
            var action:BoundAction = row.action;
            if(action.key != null)
            {
               notes.push(SelectorTexts.format(t.moves,action.label,action.key));
            }
            var losers:Array = [];
            for each(var other:BoundAction in this.session.onKey())
            {
               if(action.takesFrom(other))
               {
                  losers.push(other.label);
               }
            }
            if(losers.length > 0)
            {
               notes.push(SelectorTexts.format(t.loses,losers.join(", ")));
            }
         }
         this.warning.text = notes.join(" ");
      }

      // ---- scrolling

      private function maxScroll() : Number
      {
         var content:Number = 0;
         for each(var row:SelectorRow in this.session.rows)
         {
            content += row.rowHeight;
         }
         return Math.max(0,content - this.listHeight);
      }

      private function scrollTo(value:Number) : void
      {
         var max:Number = this.maxScroll();
         this.session.scroll = Math.round(Math.max(0,Math.min(max,value)));
         this.list.y = LIST_Y - this.session.scroll;
         // only the rows in view are drawn (each stock row's texts carry their shadow filters)
         for each(var row:SelectorRow in this.session.rows)
         {
            row.visible = row.y + row.rowHeight > this.session.scroll && row.y < this.session.scroll + this.listHeight;
         }
         this.scrollbar.visible = max > 0;
         this.scrollbar.setScrollProperties(this.listHeight,0,max,ActionRow.HEIGHT);
         this.scrollbar.position = this.session.scroll;
      }

      // `row` within the list's view (with the heading over it when it is its kind's first).
      private function reveal(row:SelectorRow, whole:Boolean = false) : void
      {
         if(row == null)
         {
            this.scrollTo(whole ? 0 : this.session.scroll);
            return;
         }
         var top:Number = row.y;
         var index:int = this.session.rows.indexOf(row);
         if(index > 0 && this.session.rows[index - 1] is GroupHeading)
         {
            top = this.session.rows[index - 1].y;
         }
         var scroll:Number = this.session.scroll;
         if(top < scroll)
         {
            scroll = top;
         }
         else if(row.y + row.rowHeight > scroll + this.listHeight)
         {
            scroll = row.y + row.rowHeight - this.listHeight;
         }
         if(whole)
         {
            // opened or filtered: the row in the middle of the view
            scroll = row.y + row.rowHeight / 2 - this.listHeight / 2;
         }
         this.scrollTo(scroll);
      }

      // ---- events

      private function onSearch(e:Event = null) : void
      {
         this.placeholder.visible = this.search.text.length == 0;
         this.session.highlighted = null;
         this.session.scroll = 0;
         this.fill(this.texts != null ? this.texts : new SelectorTexts(null));
      }

      private function onScrollBar(e:Event) : void
      {
         if(this.session != null && this.scrollbar.position != this.session.scroll)
         {
            this.scrollTo(this.scrollbar.position);
         }
      }

      private function onKey(e:KeyboardEvent) : void
      {
         if(this.session == null)
         {
            return;
         }
         // at the stage itself (nothing focused) only in its own phase; inside, in the capture phase
         if(e.eventPhase == EventPhase.BUBBLING_PHASE)
         {
            return;
         }
         if(NAVIGATION.indexOf(e.keyCode) == -1)
         {
            if(stage.focus != this.search)
            {
               stage.focus = this.search;
            }
            return;
         }
         e.stopImmediatePropagation();
         if(e.type != KeyboardEvent.KEY_DOWN)
         {
            return;
         }
         var page:int = Math.max(1,Math.floor(this.listHeight / ActionRow.HEIGHT) - 1);
         switch(e.keyCode)
         {
            case Keyboard.UP:
               this.move(-1);
               break;
            case Keyboard.DOWN:
               this.move(1);
               break;
            case Keyboard.PAGE_UP:
               this.move(-page);
               break;
            case Keyboard.PAGE_DOWN:
               this.move(page);
               break;
            case Keyboard.ENTER:
               this.choose(this.session.highlighted);
               break;
            case Keyboard.ESCAPE:
               this.cancel();
         }
      }

      // The typing's own keys go no further than the window (the menu's Esc, CLIK's focus moves).
      private function keep(e:KeyboardEvent) : void
      {
         e.stopPropagation();
      }

      private function onResize(e:Event) : void
      {
         if(this.session != null)
         {
            this.build();
            stage.focus = this.search;
         }
      }

      private function onBackdrop(e:MouseEvent) : void
      {
         this.cancel();
      }

      private function onPanelClick(e:MouseEvent) : void
      {
         if(this.session != null && stage.focus != this.search)
         {
            stage.focus = this.search;
         }
      }

      private function rowAt(e:MouseEvent) : SelectorRow
      {
         var target:DisplayObject = e.target as DisplayObject;
         while(target != null && !(target is SelectorRow))
         {
            target = target.parent;
         }
         return target as SelectorRow;
      }

      private function onRowMove(e:MouseEvent) : void
      {
         var row:SelectorRow = this.rowAt(e);
         if(row != null && row.choice && row != this.session.highlighted)
         {
            this.light(row);
         }
      }

      private function onRowClick(e:MouseEvent) : void
      {
         e.stopPropagation();
         this.choose(this.rowAt(e));
      }

      private function onWheel(e:MouseEvent) : void
      {
         this.scrollTo(this.session.scroll - e.delta * 16);
      }

      private function sound(id:int) : void
      {
         try
         {
            ExternalInterface.call("sound_play",id);
         }
         catch(error:Error)
         {
         }
      }
   }
}

import survarium.menu.selector.BoundAction;
import survarium.menu.selector.SelectorRow;

// The open window: the key's text, the actions, the host's answer, the rows shown, the row
// highlighted and the list's scroll.
class Session
{

   public var keyName:String;

   public var actions:Array;

   public var answer:Function;

   public var rows:Array = [];

   public var highlighted:SelectorRow = null;

   public var scroll:Number = 0;

   public function Session(keyName:String, actions:Array, answer:Function)
   {
      super();
      this.keyName = keyName;
      this.actions = actions;
      this.answer = answer;
   }

   // The key's actions now (a lobby one and one of the match at most, in the table's order).
   public function onKey() : Array
   {
      return this.actions.filter(function(action:BoundAction, ... rest):Boolean
      {
         return action.onKey;
      });
   }

   // KINDS' order, then the actions of no kind.
   public function kinds() : Array
   {
      var kinds:Array = [];
      for(var k:int = 0; k < SrpKeysWindow.KINDS.length; k++)
      {
         kinds.push(k);
      }
      kinds.push(-1);
      return kinds;
   }

   // A kind's actions in the rows' order (the exe's table, which the Key bindings window lists in too).
   public function ofKind(kind:int) : Array
   {
      return this.actions.filter(function(action:BoundAction, ... rest):Boolean
      {
         return action.kind == kind;
      });
   }
   
   public function choices() : Array
   {
      return this.rows.filter(function(row:SelectorRow, ... rest):Boolean
      {
         return row.choice;
      });
   }
}
