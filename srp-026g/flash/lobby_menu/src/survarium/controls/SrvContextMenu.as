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
      
      public static function show(param1:Array, param2:int = 0, param3:int = 0) : void
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
         _stage.addChild(_id);
         _id.addListeners();
         var _loc4_:Array = [];
         var _loc5_:int = 0;
         while(_loc5_ < param1.length)
         {
            _loc4_[_loc5_] = param1[_loc5_].name;
            _functions[_loc5_] = param1[_loc5_].click_function;
            _loc5_++;
         }
         var _loc6_:uint = 10;
         var _loc7_:ContextList = _id.contextList;
         _loc7_.x = _stage.mouseX - _loc6_ / 2;
         _loc7_.y = _stage.mouseY - _loc6_ - 2;
         if(_loc7_.x < 0)
         {
            _loc7_.x = 4;
         }
         if(_loc7_.y < 0)
         {
            _loc7_.y = 4;
         }
         _loc7_.setPatternt(_pattern);
         _loc7_.data = param1;
         trace("contextShowed::",_loc7_.width);
         _stage.focus = _id.contextList.list;
         if(_loc7_.x + _loc7_.width > _stage.stageWidth)
         {
            _loc7_.x = _stage.mouseX - _loc7_.width + 4;
         }
         if(_loc7_.y + _loc7_.height > _stage.stageHeight)
         {
            _loc7_.y = _stage.mouseY - _loc7_.height + 4 + 25;
         }
      }
      
      public static function setPatternt(param1:BitmapData) : void
      {
         if(_pattern == param1)
         {
            return;
         }
         _pattern = param1;
      }
      
      public static function registerStage(param1:Stage) : void
      {
         if(_stage == param1)
         {
            return;
         }
         _stage = param1;
         _stage.addEventListener(MouseEvent.MOUSE_WHEEL,hideMe,true);
      }
      
      public static function hide() : void
      {
         trace("hide",_stage.focus);
         _stage.removeChild(_id);
         _stage.removeEventListener(MouseEvent.MOUSE_DOWN,hideMe);
         if(_subMenu)
         {
            _id.removeChild(_subMenu);
            _subMenu.list.dataProvider = null;
            _subMenu = null;
         }
         _id.removeListeners();
      }
      
      protected static function hideMe(param1:MouseEvent) : void
      {
         if(_id && param1.target != _id && _id.parent && param1.currentTarget != _id && !(_id as DisplayObjectContainer).contains(param1.target as DisplayObject))
         {
            hide();
         }
      }
      
      private function onMouse(param1:MouseEvent) : void
      {
         trace("onMouseEvent::",param1.target,param1.currentTarget,param1.relatedObject);
         var _loc2_:Button = param1.target as Button;
         if(!_loc2_)
         {
            return;
         }
         trace("button.y",_loc2_.y,_loc2_.owner);
         if(_loc2_.data && _loc2_.data.options)
         {
            this.addSubMenu(_loc2_.data.options,_loc2_.y / 23);
         }
         else if(_subMenu && !_subMenu.contains(_loc2_))
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
      
      protected function addSubMenu(param1:Array, param2:int) : void
      {
         trace("addSubMenu",param2);
         if(_subMenu)
         {
            removeChild(_subMenu);
            _subMenu.list.dataProvider = null;
            _subMenu = null;
         }
         _subMenu = new _subMenuRenderer();
         _subMenu.x = this.contextList.x + this.contextList.width - 8;
         _subMenu.data = param1;
         param2 = this.contextList.y + param2 * 25 - 4;
         if(param2 + _subMenu.height > (stage ? stage.stageHeight : 0))
         {
            param2 -= (param1.length - 1) * 25 - 4;
         }
         _subMenu.y = param2;
         addChild(_subMenu);
      }
      
      protected function onSelect(param1:Event = null) : void
      {
         trace("onSelect",param1.target,param1.type);
         var _loc2_:Object = param1.target.data;
         var _loc3_:Function;
         if(_loc2_.options)
         {
            return;
         }
         hide();
         if(_loc2_.callBack)
         {
            _loc3_ = _loc2_.click_function;
            if(_loc3_ is Function)
            {
               _loc3_(_loc2_);
            }
         }
         else
         {
            _loc3_ = _loc2_.click_function;
            if(_loc3_ is Function)
            {
               _loc3_();
            }
         }
      }
   }
}

