package survarium.menu.selector
{
   import flash.display.DisplayObject;
   import flash.display.GradientType;
   import flash.display.Graphics;
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
   import flash.text.TextFieldType;
   import flash.text.TextFormat;
   import flash.ui.Keyboard;
   import survarium.constants.ActionGroup;
   import survarium.constants.UiSound;

   // The Controls' action selector (the owner, 2026-11-23: "selector should be its own thing"): a
   // panel over the menu, opened for one key, that lists every action the player may give a key,
   // by group in its colour, each with its key now; a search field filters them as one types;
   // "Unbind" leaves the key with none. Up and Down (Page Up, Page Down) move through the choices,
   // Enter or a click picks, Esc, the close button or a click outside cancels. The footer shows
   // what the pick does, and in amber when it moves an action from another key or takes the key
   // from another action (the game's conflict rule: one lobby action and one of the match a key).
   //
   // The host (the Controls window) calls
   //    open(key, bindings, onPick, onUnbind, onCancel)
   // with the key's name as the rows name keys, the options' rows (root.set_keybindings' objects:
   // the key's actions now are the rows whose key is `key`) and three functions, of which exactly
   // one is called, once, after the panel has closed:
   //    onPick(actionId:uint)   the action picked for the key
   //    onUnbind()              the key is to have no action
   //    onCancel()              nothing changes
   // and applies the pick through the game's own binding path. close() takes the panel away
   // calling none (the host closing over it); isOpen says whether it is open.
   public class ActionSelector extends Sprite
   {

      public static const WIDTH:Number = 620;

      public static const MAX_HEIGHT:Number = 720;

      // around the panel, to the screen's edges at the least
      private static const MARGIN:Number = 24;

      private static const PAD:Number = 24;

      private static const KEYCAP:Number = 64;

      private static const SEARCH_HEIGHT:Number = 42;

      private static const FOOTER:Number = 118;

      private static const LIST_X:Number = 14;

      private static const ROW_WIDTH:Number = WIDTH - LIST_X - 30;

      private static const NAVIGATION:Array = [Keyboard.UP,Keyboard.DOWN,Keyboard.PAGE_UP,Keyboard.PAGE_DOWN,Keyboard.ENTER,Keyboard.ESCAPE,Keyboard.TAB];

      // The exe's texts; null until it answers.
      private var texts:SelectorTexts = null;

      // The open panel's state; null while closed.
      private var session:Session = null;

      private var backdrop:Sprite;

      public var panel:Sprite;

      public var search:TextField;

      private var placeholder:TextField;

      private var searchBox:Shape;

      public var list:Sprite;

      private var listMask:Shape;

      private var listTop:Number;

      private var listHeight:Number;

      private var scrollbar:Sprite;

      private var thumb:Sprite;

      private var empty:TextField;

      public var footer:Sprite;

      public var warning:TextField;

      public var hints:Sprite;

      private var dragFrom:Number = NaN;

      public function ActionSelector()
      {
         super();
         visible = false;
      }

      public function get isOpen() : Boolean
      {
         return this.session != null;
      }

      // The rows on screen, top to bottom: GroupHeading, ActionRow and UnbindRow.
      public function get rows() : Array
      {
         return this.session == null ? [] : this.session.rows.concat();
      }

      // The choice the keys and the mouse are on (null: none, nothing matches).
      public function get highlighted() : SelectorRow
      {
         return this.session == null ? null : this.session.highlighted;
      }

      public function get scroll() : Number
      {
         return this.session == null ? 0 : this.session.scroll;
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

      public function open(key:String, bindings:Array, onPick:Function, onUnbind:Function, onCancel:Function) : void
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
         this.session = new Session(key,BoundAction.parse(bindings),onPick,onUnbind,onCancel);
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
         stage.removeEventListener(MouseEvent.MOUSE_MOVE,this.onThumbMove);
         stage.removeEventListener(MouseEvent.MOUSE_UP,this.onThumbUp);
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

      // The filter as the search field holds it (the harness types through it too).
      public function filter(text:String) : void
      {
         if(this.session == null)
         {
            return;
         }
         this.search.text = text;
         this.onSearch();
      }

      // Up and Down: the next choice that way, `steps` of them.
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

      // Enter or a click: the session ends with `row`'s choice.
      public function choose(row:SelectorRow) : void
      {
         var session:Session = this.session;
         if(session == null || row == null || !row.choice)
         {
            return;
         }
         this.sound(UiSound.CLICK);
         this.close();
         if(row is ActionRow)
         {
            session.onPick(ActionRow(row).action.id);
         }
         else
         {
            session.onUnbind();
         }
      }

      public function cancel() : void
      {
         var session:Session = this.session;
         if(session == null)
         {
            return;
         }
         this.close();
         session.onCancel();
      }

      // ---- the panel

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
         var height:Number = Math.round(Math.max(360,Math.min(MAX_HEIGHT,to.y - from.y - 2 * MARGIN)));
         this.backdrop = new Sprite();
         this.backdrop.graphics.beginFill(0x050607,0.62);
         this.backdrop.graphics.drawRect(from.x,from.y,to.x - from.x,to.y - from.y);
         this.backdrop.graphics.endFill();
         this.backdrop.addEventListener(MouseEvent.CLICK,this.onBackdrop);
         addChild(this.backdrop);
         this.panel = new Sprite();
         this.panel.name = "_srp_action_selector";
         this.panel.x = Math.round((from.x + to.x - WIDTH) / 2);
         this.panel.y = Math.round((from.y + to.y - height) / 2);
         addChild(this.panel);
         this.drawPanel(height);
         this.buildHeader(t);
         this.buildSearch(t,query);
         this.listTop = PAD + KEYCAP + 18 + SEARCH_HEIGHT + 12;
         this.listHeight = height - FOOTER - this.listTop - 4;
         this.list = new Sprite();
         this.list.x = LIST_X;
         this.list.y = this.listTop;
         this.listMask = new Shape();
         this.listMask.graphics.beginFill(0);
         this.listMask.graphics.drawRect(0,this.listTop,WIDTH,this.listHeight);
         this.listMask.graphics.endFill();
         this.panel.addChild(this.listMask);
         this.panel.addChild(this.list);
         this.list.mask = this.listMask;
         this.list.addEventListener(MouseEvent.MOUSE_MOVE,this.onRowMove);
         this.list.addEventListener(MouseEvent.CLICK,this.onRowClick);
         this.panel.addEventListener(MouseEvent.MOUSE_WHEEL,this.onWheel);
         this.empty = Paint.text(16,Paint.DIM);
         this.empty.text = t.noMatch;
         this.empty.x = Math.round((WIDTH - this.empty.width) / 2);
         this.empty.y = this.listTop + 40;
         this.panel.addChild(this.empty);
         this.scrollbar = new Sprite();
         this.scrollbar.x = WIDTH - 20;
         this.scrollbar.y = this.listTop;
         this.panel.addChild(this.scrollbar);
         this.thumb = new Sprite();
         this.thumb.addEventListener(MouseEvent.MOUSE_DOWN,this.onThumbDown);
         this.scrollbar.addChild(this.thumb);
         this.footer = new Sprite();
         this.footer.y = height - FOOTER;
         this.footer.mouseEnabled = this.footer.mouseChildren = false;
         this.panel.addChild(this.footer);
         this.buildHints(t);
         this.fill(t);
      }

      private function drawPanel(height:Number) : void
      {
         var g:Graphics = this.panel.graphics;
         // a soft shadow under it
         for(var i:int = 14; i >= 1; i--)
         {
            g.beginFill(0,0.035);
            g.drawRoundRect(-i,-i + 8,WIDTH + 2 * i,height + 2 * i,28 + 2 * i,28 + 2 * i);
            g.endFill();
         }
         var m:Matrix = new Matrix();
         m.createGradientBox(WIDTH,height,Math.PI / 2);
         g.lineStyle(1,Paint.EDGE,1,true);
         g.beginGradientFill(GradientType.LINEAR,[0x1A1D21,Paint.PANEL,0x121417],[1,1,1],[0,90,255],m);
         g.drawRoundRect(0.5,0.5,WIDTH - 1,height - 1,28,28);
         g.endFill();
         g.lineStyle();
         // the footer's rule
         g.beginFill(Paint.EDGE);
         g.drawRect(PAD,height - FOOTER,WIDTH - 2 * PAD,1);
         g.endFill();
         // a click on the panel is no click outside
         this.panel.addEventListener(MouseEvent.CLICK,this.onPanelClick);
      }

      private function buildHeader(t:SelectorTexts) : void
      {
         var key:String = this.session.key;
         var now:Array = this.session.actionsOn(key);
         var cap:Sprite = Paint.keycap(key,KEYCAP,26,now.length > 0 ? int(BoundAction(now[0]).group.colour) : -1);
         cap.name = "_key";
         cap.x = PAD;
         cap.y = PAD;
         this.panel.addChild(cap);
         var left:Number = cap.x + cap.width + 18;
         var room:Number = WIDTH - PAD - 40 - left;
         var title:TextField = Paint.text(13,Paint.DIM,1.4);
         Paint.fit(title,t.title.toUpperCase(),room);
         title.x = left;
         title.y = PAD + 4;
         this.panel.addChild(title);
         var line:Sprite = new Sprite();
         line.name = "_now";
         line.x = left;
         line.y = PAD + 26;
         this.panel.addChild(line);
         if(now.length == 0)
         {
            var free:TextField = Paint.text(19,Paint.DIM);
            free.text = t.free;
            line.addChild(free);
         }
         var x:Number = 0;
         for(var i:int = 0; i < now.length; i++)
         {
            var action:BoundAction = now[i];
            line.graphics.beginFill(action.group.colour);
            line.graphics.drawCircle(x + 5,15,5);
            line.graphics.endFill();
            var label:TextField = Paint.text(19,Paint.INK);
            // the last fits in what is left
            var share:Number = i == now.length - 1 ? room - x - 16 : (room - x - 16) / (now.length - i) - 20;
            Paint.fit(label,action.label,share);
            label.x = x + 16;
            line.addChild(label);
            x = label.x + label.width + 20;
         }
         var shut:Sprite = new Sprite();
         shut.name = "_close";
         shut.x = WIDTH - PAD - 30;
         shut.y = PAD - 4;
         this.drawClose(shut,false);
         shut.buttonMode = true;
         shut.addEventListener(MouseEvent.ROLL_OVER,function(e:MouseEvent):void
         {
            drawClose(shut,true);
         });
         shut.addEventListener(MouseEvent.ROLL_OUT,function(e:MouseEvent):void
         {
            drawClose(shut,false);
         });
         shut.addEventListener(MouseEvent.CLICK,function(e:MouseEvent):void
         {
            e.stopPropagation();
            cancel();
         });
         this.panel.addChild(shut);
      }

      private function drawClose(shut:Sprite, over:Boolean) : void
      {
         var g:Graphics = shut.graphics;
         g.clear();
         g.beginFill(over ? Paint.KEY_TOP : Paint.PANEL,over ? 1 : 0.004);
         g.drawRoundRect(0,0,30,30,10,10);
         g.endFill();
         g.lineStyle(2,over ? Paint.INK : Paint.DIM,1,true);
         g.moveTo(10,10);
         g.lineTo(20,20);
         g.moveTo(20,10);
         g.lineTo(10,20);
      }

      private function buildSearch(t:SelectorTexts, query:String) : void
      {
         var top:Number = PAD + KEYCAP + 18;
         this.searchBox = new Shape();
         this.searchBox.y = top;
         this.panel.addChild(this.searchBox);
         var icon:Shape = new Shape();
         icon.graphics.lineStyle(2,Paint.DIM,1,true);
         icon.graphics.drawCircle(PAD + 20,top + 19,6.5);
         icon.graphics.moveTo(PAD + 24.8,top + 23.8);
         icon.graphics.lineTo(PAD + 29,top + 28);
         this.panel.addChild(icon);
         this.placeholder = Paint.text(17,Paint.FAINT);
         Paint.fit(this.placeholder,t.search,WIDTH - 2 * PAD - 56);
         this.search = new TextField();
         this.search.name = "_search";
         this.search.type = TextFieldType.INPUT;
         this.search.embedFonts = true;
         this.search.multiline = false;
         this.search.wordWrap = false;
         this.search.maxChars = 40;
         this.search.defaultTextFormat = Paint.format(17,Paint.INK);
         this.search.width = WIDTH - 2 * PAD - 56;
         this.search.height = 26;
         this.search.text = query;
         this.search.setSelection(query.length,query.length);
         this.search.x = this.placeholder.x = PAD + 40;
         this.search.y = top + Math.round((SEARCH_HEIGHT - 26) / 2);
         this.placeholder.y = this.search.y + 2;
         this.panel.addChild(this.placeholder);
         this.panel.addChild(this.search);
         this.search.addEventListener(Event.CHANGE,this.onSearch);
         this.drawSearch(true);
      }

      private function drawSearch(focused:Boolean) : void
      {
         var g:Graphics = this.searchBox.graphics;
         g.clear();
         g.lineStyle(1,focused ? Paint.mix(Paint.EDGE,Paint.INK,0.28) : Paint.EDGE,1,true);
         g.beginFill(Paint.SHADE);
         g.drawRoundRect(PAD + 0.5,0.5,WIDTH - 2 * PAD - 1,SEARCH_HEIGHT - 1,20,20);
         g.endFill();
         this.placeholder.visible = this.search.text.length == 0;
      }

      // The keys' hints at the footer's bottom right: Up/Down choose, Enter assign, Esc cancel.
      private function buildHints(t:SelectorTexts) : void
      {
         this.hints = new Sprite();
         var x:Number = 0;
         var pairs:Array = [[null,t.choose],["Enter",t.assign],["Esc",t.cancel]];
         for each(var pair:Array in pairs)
         {
            var cap:Sprite = Paint.keycap(pair[0] == null ? "" : pair[0],22,12);
            if(pair[0] == null)
            {
               Paint.arrow(cap.graphics,6,6,10,true,Paint.DIM);
               var down:Sprite = Paint.keycap("",22,12);
               Paint.arrow(down.graphics,6,8,10,false,Paint.DIM);
               down.x = x + 26;
               this.hints.addChild(down);
            }
            cap.x = x;
            this.hints.addChild(cap);
            x += pair[0] == null ? 52 : cap.width + 6;
            var word:TextField = Paint.text(13,Paint.DIM);
            word.text = pair[1];
            word.x = x;
            word.y = Math.round((22 - word.height) / 2);
            this.hints.addChild(word);
            x = word.x + word.width + 14;
         }
         this.hints.x = WIDTH - PAD - (x - 14);
         this.hints.y = FOOTER - 34;
         this.footer.addChild(this.hints);
      }

      // The rows for the search field's text, the highlight kept where it can be.
      private function fill(t:SelectorTexts) : void
      {
         var keep:SelectorRow = this.session.highlighted;
         var keptAction:BoundAction = keep is ActionRow ? ActionRow(keep).action : null;
         var keptUnbind:Boolean = keep is UnbindRow;
         while(this.list.numChildren > 0)
         {
            this.list.removeChildAt(0);
         }
         var rows:Array = [];
         var key:String = this.session.key;
         var words:Array = this.search.text.toLowerCase().split(/\s+/).filter(function(w:String, ... rest):Boolean
         {
            return w.length > 0;
         });
         var now:Array = this.session.actionsOn(key);
         if(now.length > 0 && (words.length == 0 || matches(t.unbind,"",words)))
         {
            rows.push(new UnbindRow(key,t,ROW_WIDTH));
         }
         for each(var group:ActionGroup in ActionGroup.ALL)
         {
            var heading:GroupHeading = null;
            for each(var action:BoundAction in this.session.actions)
            {
               if(action.group != group || !matches(action.label + " " + t.groups[group.index],action.key,words))
               {
                  continue;
               }
               if(heading == null)
               {
                  heading = new GroupHeading(group,t,ROW_WIDTH);
                  rows.push(heading);
               }
               rows.push(new ActionRow(action,key,t,ROW_WIDTH));
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
         // the same choice again, else the key's action, else the first action
         var light:SelectorRow = null;
         for each(row in rows)
         {
            if(row is ActionRow && ActionRow(row).action == keptAction || row is UnbindRow && keptUnbind)
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
         if(light == null && rows.length > 0)
         {
            light = rows[0];
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
         this.preview();
      }

      // The footer: the pick's action, an arrow, the key; under it what else the pick changes.
      private function preview() : void
      {
         var t:SelectorTexts = this.texts != null ? this.texts : new SelectorTexts(null);
         for each(var part:String in ["_pick","_sign"])
         {
            var old:DisplayObject = this.footer.getChildByName(part);
            if(old != null)
            {
               this.footer.removeChild(old);
            }
         }
         if(this.warning != null && this.warning.parent != null)
         {
            this.warning.parent.removeChild(this.warning);
         }
         var row:SelectorRow = this.session.highlighted;
         if(row == null)
         {
            return;
         }
         var key:String = this.session.key;
         var pick:Sprite = new Sprite();
         pick.name = "_pick";
         pick.x = PAD;
         pick.y = 16;
         var colour:uint = row is ActionRow ? ActionRow(row).action.group.colour : Paint.UNBIND;
         var name:TextField = Paint.text(16,row is ActionRow ? Paint.INK : Paint.mix(Paint.UNBIND,Paint.INK,0.3));
         var keyCap:Sprite = Paint.keycap(key,22,12,row is ActionRow ? int(colour) : -1);
         var room:Number = WIDTH - 2 * PAD - 16 - 14 - 28 - keyCap.width;
         Paint.fit(name,row is ActionRow ? ActionRow(row).action.label : t.unbind,room);
         pick.graphics.beginFill(colour);
         pick.graphics.drawCircle(5,11,5);
         pick.graphics.endFill();
         name.x = 16;
         name.y = Math.round((22 - name.height) / 2);
         pick.addChild(name);
         // the arrow
         var ax:Number = name.x + name.width + 10;
         pick.graphics.lineStyle(2,Paint.DIM,1,true);
         pick.graphics.moveTo(ax,11);
         pick.graphics.lineTo(ax + 16,11);
         pick.graphics.lineStyle();
         pick.graphics.beginFill(Paint.DIM);
         pick.graphics.moveTo(ax + 20,11);
         pick.graphics.lineTo(ax + 13,6.5);
         pick.graphics.lineTo(ax + 13,15.5);
         pick.graphics.endFill();
         keyCap.x = ax + 28;
         pick.addChild(keyCap);
         this.footer.addChild(pick);
         var notes:Array = [];
         var losers:Array = [];
         if(row is ActionRow)
         {
            var action:BoundAction = ActionRow(row).action;
            if(action.key != null && action.key != key)
            {
               notes.push(SelectorTexts.format(t.moves,action.label,action.key));
            }
            if(action.key != key)
            {
               for each(var other:BoundAction in this.session.actionsOn(key))
               {
                  if(action.takesFrom(other))
                  {
                     losers.push(other.label);
                  }
               }
            }
         }
         else
         {
            for each(other in this.session.actionsOn(key))
            {
               losers.push(other.label);
            }
         }
         if(losers.length > 0)
         {
            notes.push(SelectorTexts.format(t.loses,losers.join(", ")));
         }
         if(notes.length == 0)
         {
            return;
         }
         if(this.warning == null)
         {
            this.warning = new TextField();
            this.warning.name = "_warning";
            this.warning.embedFonts = true;
            this.warning.selectable = false;
            this.warning.mouseEnabled = false;
            this.warning.multiline = true;
            this.warning.wordWrap = true;
            this.warning.autoSize = TextFieldAutoSize.LEFT;
            this.warning.defaultTextFormat = Paint.format(15,Paint.WARNING);
         }
         this.warning.x = PAD + 22;
         this.warning.y = 48;
         this.warning.width = WIDTH - 2 * PAD - 22;
         this.warning.text = notes.join(" ");
         this.footer.addChild(this.warning);
         // the warning's sign: a triangle with its mark
         var sign:Shape = new Shape();
         var g:Graphics = sign.graphics;
         g.beginFill(Paint.WARNING);
         g.moveTo(7,0);
         g.lineTo(14.5,13);
         g.lineTo(-0.5,13);
         g.endFill();
         g.beginFill(Paint.PANEL);
         g.drawRect(6.2,4.5,1.6,4.5);
         g.drawRect(6.2,10.2,1.6,1.6);
         g.endFill();
         sign.name = "_sign";
         sign.x = PAD;
         sign.y = this.warning.y + 3;
         this.footer.addChild(sign);
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
         this.session.scroll = Math.round(Math.max(0,Math.min(this.maxScroll(),value)));
         this.list.y = this.listTop - this.session.scroll;
         var g:Graphics = this.scrollbar.graphics;
         g.clear();
         this.thumb.graphics.clear();
         var max:Number = this.maxScroll();
         if(max <= 0)
         {
            return;
         }
         g.beginFill(Paint.KEY);
         g.drawRoundRect(0,0,5,this.listHeight,5,5);
         g.endFill();
         var length:Number = Math.max(36,this.listHeight * this.listHeight / (this.listHeight + max));
         this.thumb.graphics.beginFill(Paint.mix(Paint.FAINT,Paint.DIM,0.4));
         this.thumb.graphics.drawRoundRect(0,0,5,length,5,5);
         this.thumb.graphics.endFill();
         // the thumb's grip wider than it shows
         this.thumb.graphics.beginFill(0,0.004);
         this.thumb.graphics.drawRect(-6,0,17,length);
         this.thumb.graphics.endFill();
         this.thumb.y = Math.round((this.listHeight - length) * this.session.scroll / max);
      }

      // `row` within the list's view (with the heading over it when it is its group's first).
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
         this.drawSearch(true);
         this.session.highlighted = null;
         this.session.scroll = 0;
         this.fill(this.texts != null ? this.texts : new SelectorTexts(null));
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

      // The typing's own keys go no further than the panel (the menu's Esc, CLIK's focus moves).
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
         this.scrollTo(this.session.scroll - e.delta * 14);
      }

      private function onThumbDown(e:MouseEvent) : void
      {
         this.dragFrom = this.scrollbar.mouseY - this.thumb.y;
         stage.addEventListener(MouseEvent.MOUSE_MOVE,this.onThumbMove);
         stage.addEventListener(MouseEvent.MOUSE_UP,this.onThumbUp);
      }

      private function onThumbMove(e:MouseEvent) : void
      {
         var max:Number = this.maxScroll();
         var room:Number = this.listHeight - this.thumb.height;
         if(room > 0)
         {
            this.scrollTo((this.scrollbar.mouseY - this.dragFrom) / room * max);
         }
      }

      private function onThumbUp(e:MouseEvent) : void
      {
         stage.removeEventListener(MouseEvent.MOUSE_MOVE,this.onThumbMove);
         stage.removeEventListener(MouseEvent.MOUSE_UP,this.onThumbUp);
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

// The open panel: the key, the actions, the host's three functions, the rows shown, the choice
// highlighted and the list's scroll.
class Session
{

   public var key:String;

   public var actions:Array;

   public var onPick:Function;

   public var onUnbind:Function;

   public var onCancel:Function;

   public var rows:Array = [];

   public var highlighted:SelectorRow = null;

   public var scroll:Number = 0;

   public function Session(key:String, actions:Array, onPick:Function, onUnbind:Function, onCancel:Function)
   {
      super();
      this.key = key;
      this.actions = actions;
      this.onPick = onPick;
      this.onUnbind = onUnbind;
      this.onCancel = onCancel;
   }

   // The key's actions now (a lobby one and one of the match at most, in the table's order).
   public function actionsOn(key:String) : Array
   {
      return this.actions.filter(function(action:BoundAction, ... rest):Boolean
      {
         return action.key == key;
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
