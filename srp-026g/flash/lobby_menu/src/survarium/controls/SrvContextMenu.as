package survarium.controls
{
   import flash.display.BitmapData;
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.MovieClip;
   import flash.display.Stage;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.controls.Button;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   
   public class SrvContextMenu extends MovieClip
   {
      
      private static var _pattern:BitmapData;
      
      private static var _stage:Stage;

      // where the menu is drawn: the lobby's layers (LobbyMenu._layers), else the stage
      private static var _layer:DisplayObjectContainer;
      
      private static var _id:SrvContextMenu;
      
      private static var _functions:Array = [];
      
      private static var _menuRenderer:Class;
      
      private static var _subMenuRenderer:Class;
      
      protected static var _subMenu:ContextList;
      
      
      public var contextList:ContextList;
      
      public function SrvContextMenu()
      {
         super();
      }
      
      public static function get visisble() : Boolean
      {
         return _id ? _id.parent != null : false;
      }
      
      public static function getInstance() : SrvContextMenu
      {
         return _id;
      }
      
      public static function show(items:Array, posX:int = 0, posY:int = 0, owner:DisplayObject = null) : void
      {
         if(!_id)
         {
            _menuRenderer = getDefinitionByName("ContextMenuLobby") as Class;
            _subMenuRenderer = getDefinitionByName("SrvContextList") as Class;
            trace("SrvContextShow::",_menuRenderer,_subMenuRenderer);
            _id = new _menuRenderer();
         }
         if(!_stage)
         {
            return;
         }
         _functions.length = 0;
         if(!_id.parent)
         {
            _stage.addEventListener(MouseEvent.MOUSE_DOWN,hideMe);
         }
         _layer.addChild(_id);
         _id.addListeners();
         var names:Array = [];
         var i:int = 0;
         while(i < items.length)
         {
            names[i] = items[i].name;
            _functions[i] = items[i].click_function;
            i++;
         }
         // The menu is on the stage, drawn at the lobby's scale (2 at 4K); it is
         // placed in its own units: the mouse and the screen divided by that scale.
         var scale:Number = MainScene.overlayScale;
         _id.scaleX = _id.scaleY = scale;
         var atX:Number = _stage.mouseX / scale;
         var atY:Number = _stage.mouseY / scale;
         var offset:uint = 10;
         var list:ContextList = _id.contextList;
         list.x = atX - offset / 2;
         list.y = atY - offset - 2;
         list.setPatternt(_pattern);
         list.data = items;
         trace("contextShowed::",list.width);
         _stage.focus = _id.contextList.list;
         if(list.x + list.width > screenWidth())
         {
            list.x = atX - list.width + 4;
         }
         if(list.y + list.height > screenHeight())
         {
            list.y = atY - list.height + 4 + 25;
         }
         // Kept on the screen at its edges (a tall menu near the bottom).
         list.x = Math.max(4,Math.min(list.x,screenWidth() - list.width - 4));
         list.y = Math.max(4,Math.min(list.y,screenHeight() - list.height - 4));
         setOwner(owner);
         // a modal over the messages too: Esc hides it first
         Modals.open(_id,Modals.MENU,function():void
         {
            if(visisble)
            {
               hide();
            }
         },function():Boolean
         {
            return visisble;
         });
      }
      
      // The screen in the menu's units.
      private static function screenWidth() : Number
      {
         return _stage.stageWidth / (_id ? _id.scaleX : 1);
      }
      
      private static function screenHeight() : Number
      {
         return _stage.stageHeight / (_id ? _id.scaleY : 1);
      }
      
      public static function setPatternt(pattern:BitmapData) : void
      {
         if(_pattern == pattern)
         {
            return;
         }
         _pattern = pattern;
      }
      
      public static function registerStage(stageRef:Stage, layer:DisplayObjectContainer = null) : void
      {
         _layer = layer ? layer : stageRef;
         if(_stage == stageRef)
         {
            return;
         }
         _stage = stageRef;
         _stage.addEventListener(MouseEvent.MOUSE_WHEEL,hideMe,true);
      }
      
      public static function hide() : void
      {
         Modals.leave(_id);
         setOwner(null);
         trace("hide",_stage.focus);
         _layer.removeChild(_id);
         _stage.removeEventListener(MouseEvent.MOUSE_DOWN,hideMe);
         if(_subMenu)
         {
            _id.removeChild(_subMenu);
            _subMenu.list.dataProvider = null;
            _subMenu = null;
         }
         _id.removeListeners();
      }
      
      protected static function hideMe(event:MouseEvent) : void
      {
         if(_id && event.target != _id && _id.parent && event.currentTarget != _id && !(_id as DisplayObjectContainer).contains(event.target as DisplayObject))
         {
            hide();
         }
      }
      
      private function onMouse(event:MouseEvent) : void
      {
         trace("onMouseEvent::",event.target,event.currentTarget,event.relatedObject);
         var button:Button = event.target as Button;
         if(!button)
         {
            return;
         }
         trace("button.y",button.y,button.owner);
         if(button.data && button.data.options)
         {
            this.addSubMenu(button.data.options,button.y / 23);
         }
         else if(_subMenu && !_subMenu.contains(button))
         {
            this.removeSubMenu();
         }
      }
      
      public function addListeners() : void
      {
         addEventListener(ButtonEvent.CLICK,this.onSelect);
         addEventListener(MouseEvent.MOUSE_OVER,this.onMouse);
      }
      
      public function removeListeners() : void
      {
         removeEventListener(ButtonEvent.CLICK,this.onSelect);
         removeEventListener(MouseEvent.MOUSE_OVER,this.onMouse);
      }
      
      protected function removeSubMenu() : void
      {
         removeChild(_subMenu);
         _subMenu.list.dataProvider = new DataProvider(null);
         _subMenu = null;
      }
      
      protected function addSubMenu(options:Array, rowPos:int) : void
      {
         trace("addSubMenu",rowPos);
         if(_subMenu)
         {
            removeChild(_subMenu);
            _subMenu.list.dataProvider = null;
            _subMenu = null;
         }
         _subMenu = new _subMenuRenderer();
         _subMenu.x = this.contextList.x + this.contextList.width - 8;
         _subMenu.data = options;
         rowPos = this.contextList.y + rowPos * 25 - 4;
         if(rowPos + _subMenu.height > screenHeight())
         {
            rowPos -= (options.length - 1) * 25 - 4;
         }
         // Left of the menu when it would go past the screen's right edge.
         if(_subMenu.x + _subMenu.width > screenWidth())
         {
            _subMenu.x = this.contextList.x - _subMenu.width + 8;
         }
         _subMenu.y = Math.max(4,rowPos);
         addChild(_subMenu);
      }
      
      protected function onSelect(event:Event = null) : void
      {
         trace("onSelect",event.target,event.type);
         var itemData:Object = event.target.data;
         var callback:Function;
         if(itemData.options)
         {
            return;
         }
         hide();
         if(itemData.callBack)
         {
            callback = itemData.click_function;
            if(callback is Function)
            {
               callback(itemData);
            }
         }
         else
         {
            callback = itemData.click_function;
            if(callback is Function)
            {
               callback();
            }
         }
      }
      
      // The window a menu was opened for (show's owner): while the menu is
      // open it closes with it, however the window goes (hidden, closed,
      // taken off the stage). A menu with no owner stays as before.
      private static var _owner:DisplayObject;
      
      private static function setOwner(owner:DisplayObject) : void
      {
         if(_id == null)
         {
            return;
         }
         _id.removeEventListener(Event.ENTER_FRAME,watchOwner);
         _owner = owner;
         if(owner != null)
         {
            _id.addEventListener(Event.ENTER_FRAME,watchOwner);
         }
      }
      
      private static function watchOwner(e:Event) : void
      {
         if(_owner == null || !_id.parent)
         {
            return;
         }
         for(var at:DisplayObject = _owner; at != null; at = at.parent)
         {
            if(!at.visible)
            {
               hide();
               return;
            }
         }
         if(_owner.stage == null)
         {
            hide();
         }
      }

      // A menu still open with these items (the same objects show was given)
      // gets one more at its end, kept on the screen: an item known only
      // after the menu opened (a friend's room). False: it was closed or
      // another menu replaced it.
      public static function addItem(items:Array, item:Object) : Boolean
      {
         if(!visisble || _id.contextList == null || _id.contextList.list == null)
         {
            return false;
         }
         var shown:Object = _id.contextList.list.dataProvider;
         if(shown == null || shown.length != items.length)
         {
            return false;
         }
         for(var i:int = 0; i < items.length; i++)
         {
            if(shown[i] !== items[i])
            {
               return false;
            }
         }
         items.push(item);
         var list:ContextList = _id.contextList;
         list.data = items;
         if(_stage != null && list.y + list.height > screenHeight())
         {
            list.y = Math.max(4,screenHeight() - list.height - 4);
         }
         return true;
      }
   }
}

