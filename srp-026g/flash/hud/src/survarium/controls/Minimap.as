package survarium.controls
{
   import flash.display.Bitmap;
   import flash.display.BitmapData;
   import flash.display.MovieClip;
   import flash.geom.Point;
   import flash.geom.Rectangle;
   import flash.utils.Dictionary;
   import scaleform.clik.layout.LayoutData;
   
   public class Minimap extends MovieClip
   {
      
      public var minimap:MovieClip;
      
      public var compass:MovieClip;
      
      public var sizeBitmap:MovieClip;
      
      public var layoutData:LayoutData;
      
      public var compass_border:MovieClip;
      
      private const to_dir:Number = 57.29577951308232;
      
      private var level_03_map:BitmapData = new minimap_base();
      
      protected var _cx:Number = 1;
      
      protected var _cy:Number = 1;
      
      protected var _px:Number = 0;
      
      protected var _py:Number = 0;
      
      protected var _map_sx:Number;
      
      protected var _map_sy:Number;
      
      protected var _map_ox:Number;
      
      protected var _map_oy:Number;
      
      protected var _icons:Object = {
         "base":Base,
         "base_highlighted":BaseHighlighted,
         "base_enemy":BaseEnemy,
         "artifact":ArtifactArrow,
         "container":ArtifactArrow,
         "flag":Flag,
         "player_blue":PlayerBlue,
         "player_red":PlayerRed,
         "player_squad":PlayerSquad,
         "player_carry":PlayerCarry,
         "player_carry_squad":PlayerSquadCarry,
         "player_local":PlayerLocal,
         "battery_spawn":SpawnIcon,
         "resupply":ResupplyIcon
      };
      
      protected var _levels:Object = {
         "level_01":new minimap_rudnya(),
         "level_02":new minimap_school(),
         "level_03":level_03_map,
         "level_04":new minimap_zavod(),
         "test":level_03_map,
         "level_03_evn":level_03_map,
         "level_03_rain":level_03_map,
         "level_05":new minimap_fort(),
         "level_06":new Map_06(),
         "level_07":new Map_07(),
         "level_08":new Map_08(),
         "level_09":new Map_09(),
         "v011e_level_04":new minimap_zv11e(),
         "v011e_level_03":new minimap_bv11()
      };
      
      protected var _objects:Dictionary = new Dictionary(true);
      
      protected var _objects_static:Dictionary = new Dictionary(true);
      
      protected var _rotable:Boolean = false;
      
      protected var _my_team:int = 1;
      
      protected var _data:Object;
      
      protected var _renderers:Vector.<MinimapObject> = new Vector.<MinimapObject>();
      
      protected var _mapBitmap:Bitmap = new Bitmap();
      
      protected var _minimapProps:Object = new Object();
      
      private var _centerPlayer:Boolean = true;
      
      protected var _mapVisibleRectangle:Rectangle = null;
      
      public function Minimap()
      {
         super();
         visible = false;
         mouseChildren = false;
         mouseEnabled = false;
         this.minimap.map_ground.addChild(this._mapBitmap);
         this.minimap.map_ground.y = 0;
         this._minimapProps["default"] = {
            "size":100,
            "posX":-256,
            "posY":-256
         };
         this._minimapProps.level_01 = {
            "size":130,
            "posX":12.558,
            "posY":-25.774
         };
         this._minimapProps.level_02 = {
            "size":110,
            "posX":21.079,
            "posY":105.098
         };
         this._minimapProps.level_03 = {
            "size":100,
            "posX":14,
            "posY":-42.196
         };
         this._minimapProps.level_04 = {
            "size":115,
            "posX":15,
            "posY":43
         };
         this._minimapProps.v011e_level_04 = {
            "size":140,
            "posX":23.302,
            "posY":43.049
         };
         this._minimapProps.v011e_level_03 = {
            "size":100,
            "posX":14.23,
            "posY":-42.196
         };
         this._minimapProps.level_05 = {
            "size":90,
            "posX":-1.096,
            "posY":-14.211
         };
         this._minimapProps.level_06 = {
            "size":100,
            "posX":30,
            "posY":109
         };
         this._minimapProps.level_07 = {
            "size":100,
            "posX":0,
            "posY":0
         };
         this._minimapProps.level_08 = {
            "size":92,
            "posX":0,
            "posY":35
         };
         this._minimapProps.level_09 = {
            "size":140,
            "posX":0,
            "posY":-5
         };
         this._minimapProps.level_03_rain = this._minimapProps.level_03;
         this._minimapProps.level_03_evn = this._minimapProps.level_03;
      }
      
      override public function get width() : Number
      {
         return this.compass.width;
      }
      
      override public function get height() : Number
      {
         return this.compass.height;
      }
      
      public function setPlayersData(value:Object) : void
      {
         this.setData(value);
      }
      
      public function setObjectData(value:Object) : void
      {
         var _obj:Object = null;
         var i:int = 0;
         while(i < value.length)
         {
            _obj = value[i];
            if(!this._objects_static[_obj.id] || !(this._objects_static[_obj.id] is (this._icons[_obj.type] as Class)))
            {
               this.registerObject(_obj.id,_obj.type,_obj.pos_x,_obj.pos_y,this._objects_static);
            }
            else
            {
               this.updateObjectPos(_obj.id,_obj.pos_x,_obj.pos_y,0,this._objects_static);
            }
            i++;
         }
         this.clear_objects(this._objects_static);
         this.update_objects();
      }
      
      public function clear_minimap() : void
      {
         var _inst:MinimapObject = null;
         this._objects = new Dictionary(true);
         this._objects_static = new Dictionary(true);
         var i:int = 0;
         while(i < this._renderers.length)
         {
            _inst = this._renderers[i];
            if(_inst is MinimapObjectBasix)
            {
               this.minimap.removeChild(_inst);
            }
            else
            {
               this.minimap._players.removeChild(_inst);
            }
            i++;
         }
      }
      
      private function setData(value:Object) : void
      {
         var _obj:Object = null;
         var _inst:MinimapObjectPlayer = null;
         var tp:String = null;
         this._data = value;
         var l:uint = uint(this._data.length);
         trace("players::",l,">>>>>>>>>>>>>>>>\n");
         var i:int = 0;
         while(i < l)
         {
            _obj = this._data[i];
            _inst = this._objects[_obj.player_id];
            if(_obj.is_carrying_item == undefined)
            {
               _obj.is_carrying_item = false;
            }
            if(Boolean(_inst) && _obj.is_carrying_item != _inst.isCarryingItem)
            {
               trace("clearObject::",_obj.player_id,i,_obj.is_carrying_item,_inst.isCarryingItem);
               this.clear_object(_obj.player_id);
            }
            if(!this._objects[_obj.player_id])
            {
               if(_obj.team == this._my_team && Boolean(_obj.is_local_player))
               {
                  tp = "player_local";
               }
               else if(_obj.team == this._my_team && Boolean(_obj.is_carrying_item))
               {
                  tp = _obj.is_squad_player == true ? "player_carry_squad" : "player_carry";
               }
               else if(_obj.is_squad_player == true)
               {
                  tp = "player_squad";
               }
               else
               {
                  tp = _obj.team == this._my_team ? "player_blue" : "player_red";
               }
               trace("register::",_obj.player_id,tp,_obj.team,this._my_team,_obj.type);
               this.registerObject(_obj.player_id,tp,_obj.player_pos_x,_obj.player_pos_y,this._objects,_obj.type,_obj.is_carrying_item);
            }
            else
            {
               trace("update::",_obj.player_id);
               this.updateObjectPos(_obj.player_id,_obj.player_pos_x,_obj.player_pos_y,0,this._objects);
            }
            i++;
         }
         this.clear_objects(this._objects);
         this.update_objects();
      }
      
      private function clear_object(target:int) : void
      {
         var _inst:MinimapObject = null;
         if(Boolean(this._objects[target]))
         {
            _inst = this._objects[target];
            delete this._objects[target];
            this._renderers.splice(this._renderers.indexOf(_inst),1);
            if(_inst is MinimapObjectBasix)
            {
               this.minimap.removeChild(_inst);
            }
            else
            {
               this.minimap._players.removeChild(_inst);
            }
         }
      }
      
      private function clear_objects(target:Dictionary) : void
      {
         var _inst:MinimapObject = null;
         var instPos:int = 0;
         var l:uint = 0;
         var i:int = 0;
         var _objects:Dictionary = target;
         trace("clear_objects:: _renderers",this._renderers.length);
         var toDelete:Array = null;
         for each(_inst in _objects)
         {
            if(!_inst.updated)
            {
               if(!toDelete)
               {
                  toDelete = [];
               }
               toDelete.push(_inst.index);
               instPos = this._renderers.indexOf(_inst);
               if(instPos != -1)
               {
                  this._renderers.splice(instPos,1);
               }
               if(_inst is MinimapObjectBasix)
               {
                  this.minimap.removeChild(_inst);
               }
               else
               {
                  this.minimap._players.removeChild(_inst);
               }
            }
         }
         if(Boolean(toDelete))
         {
            l = toDelete.length;
            i = 0;
            while(i < l)
            {
               delete _objects[toDelete[i]];
               i++;
            }
         }
      }
      
      private function angle_diff(a:int, n:int) : int
      {
         var angle:int = 0;
         angle = a - n;
         if(angle > 180)
         {
            angle -= 360;
         }
         else if(angle < -180)
         {
            angle += 360;
         }
         return angle;
      }
      
      public function get centerPlayer() : Boolean
      {
         return this._centerPlayer;
      }
      
      public function set centerPlayer(value:Boolean) : void
      {
         this._centerPlayer = value;
      }
      
      public function get minimapRotation() : Number
      {
         return this.minimap.rotation;
      }
      
      public function set minimapRotation(value:Number) : void
      {
         this.minimap.rotation = int(value);
         this.minimap.player.rotation = -this.minimap.rotation;
         this.compass_border.rotation = this.minimap.rotation;
         this.update_objectsRotation();
      }
      
      public function get playerX() : Number
      {
         return this._px / this._cx;
      }
      
      public function set playerX(_x:Number) : void
      {
         this._px = _x * this._cx;
         this.update_objects();
      }
      
      public function get playerY() : Number
      {
         return this._py / this._cy;
      }
      
      public function set playerY(_y:Number) : void
      {
         this._py = _y * this._cy;
         this.update_objects();
      }
      
      public function setRotable(value:Boolean) : void
      {
         this._rotable = value;
         if(!value)
         {
            this.minimapRotation = 0;
         }
      }
      
      public function setProps(value:Object) : void
      {
         var _obj:Object = null;
         var max_map_size:int = 0;
         visible = true;
         trace("*************setProps::",value.map);
         if(!this._minimapProps[value.map])
         {
            value.map = "level_09";
         }
         if(Boolean(this._minimapProps[value.map]))
         {
            _obj = this._minimapProps[value.map];
         }
         else
         {
            _obj = this._minimapProps["default"];
         }
         this._map_sx = _obj.size * 2;
         this._map_sy = _obj.size * 2;
         var _mapb:BitmapData = this._levels[value.map];
         this._cx = _mapb.width / this._map_sx;
         this._cy = _mapb.height / this._map_sy;
         if(this._centerPlayer)
         {
            this._map_ox = (_obj.posX - _obj.size) * this._cx;
            this._map_oy = (-_obj.posY - _obj.size) * this._cy;
         }
         else
         {
            this._map_ox = (_obj.posX - _obj.size) * this._cx;
            this._map_oy = (-_obj.posY - _obj.size) * this._cy;
         }
         max_map_size = 610;
         var max_size:int = Math.sqrt(max_map_size * max_map_size + max_map_size * max_map_size) * 2;
         if(Boolean(this.sizeBitmap))
         {
            this.sizeBitmap.width = max_size;
            this.sizeBitmap.height = max_size;
         }
         this._mapVisibleRectangle = _mapb.getColorBoundsRect(16777215,0,false);
         this._mapBitmap.bitmapData = _mapb;
      }
      
      public function getMapSize() : Point
      {
         if(!this._mapBitmap.bitmapData)
         {
            return new Point();
         }
         return new Point(this._mapBitmap.bitmapData.width,this._mapBitmap.bitmapData.height);
      }
      
      public function getVisibleRectangle() : Rectangle
      {
         if(Boolean(this._mapVisibleRectangle) && Boolean(this._mapBitmap.bitmapData) && this._mapVisibleRectangle.width == 0)
         {
            this._mapVisibleRectangle = this._mapBitmap.bitmapData.rect;
            trace("new_mapVisibleRectangle",this._mapVisibleRectangle);
         }
         else
         {
            trace("cur_mapVisibleRectangle",this._mapVisibleRectangle);
         }
         return this._mapVisibleRectangle;
      }
      
      public function updatePlayerPos(posX:Number, posY:Number, rot:Number, team:uint = 23) : void
      {
         this._my_team = team;
         if(this._centerPlayer)
         {
            this._px = posX * this._cx;
            this._py = posY * this._cy;
         }
         else
         {
            this.minimap.player.x = -this._map_ox + posX * this._cx;
            this.minimap.player.y = -this._map_oy + posY * this._cy;
            this._px = posX * this._cx;
            this._py = posY * this._cy;
         }
         if(this._rotable)
         {
            this.minimapRotation = rot * this.to_dir;
         }
         else
         {
            this.minimap.player.rotation = -rot * this.to_dir;
         }
         this.update_objects();
      }
      
      public function update_objectsRotation() : void
      {
         var _inst:MinimapObject = null;
         var i:int = 0;
         while(i < this._renderers.length)
         {
            if(this._renderers[i])
            {
               _inst = this._renderers[i];
               _inst.rotation = -this.minimap.rotation + _inst.rotation_value;
            }
            i++;
         }
         this.minimap.player.rotation = -this.minimap.rotation;
      }
      
      public function updateObjectPos(_id:int, posX:Number, posY:Number, rot:Number, target:Dictionary) : void
      {
         var _objects:Dictionary = target;
         var _inst:MinimapObject = _objects[_id];
         _inst.xx = posX;
         _inst.yy = posY;
         _inst.rotation_value = -rot * this.to_dir;
         _inst.updated = true;
      }
      
      private function point_direction(x1:Number, y1:Number, x2:Number, y2:Number) : Number
      {
         return Math.atan2(y1 - y2,x2 - x1);
      }
      
      private function len_x(len:Number, dir:Number) : Number
      {
         return -len * Math.cos(dir);
      }
      
      private function len_y(len:Number, dir:Number) : Number
      {
         return len * Math.sin(dir);
      }
      
      private function point_distance(x1:Number, y1:Number, x2:Number, y2:Number) : Number
      {
         return (x1 - x2) * (x1 - x2) + (y1 - y2) * (y1 - y2);
      }
      
      protected function update_objects() : void
      {
         var _inst:MinimapObject = null;
         var _dir:Number = NaN;
         if(this._centerPlayer)
         {
            this.minimap.map_ground.x = -this._px + this._map_ox;
            this.minimap.map_ground.y = -this._py + this._map_oy;
         }
         else
         {
            this.minimap.map_ground.x = 0;
            this.minimap.map_ground.y = 0;
         }
         var i:int = 0;
         while(i < this._renderers.length)
         {
            _inst = this._renderers[i];
            if(this._centerPlayer)
            {
               _inst.x = _inst.xx * this._cx - this._px;
               _inst.y = _inst.yy * this._cy - this._py;
            }
            else
            {
               _inst.x = -this._map_ox + _inst.xx * this._cx;
               _inst.y = -this._map_oy + _inst.yy * this._cy;
            }
            trace("updatePosistion::",_inst);
            _inst.updated = false;
            if(this._centerPlayer)
            {
               if(_inst is MinimapObjectBasix && this.point_distance(_inst.x,_inst.y,0,0) > 11025)
               {
                  _dir = this.point_direction(_inst.x,_inst.y,0,0);
                  _inst.x = this.len_x(105,_dir);
                  _inst.y = this.len_y(105,_dir);
               }
            }
            i++;
         }
      }
      
      protected function changeObject(_id:int, type_nm:String) : void
      {
         var _inst:MinimapObject = this._objects[_id];
         if(_inst is MinimapObjectBasix)
         {
            this.minimap.removeChild(_inst);
         }
         else
         {
            this.minimap._players.removeChild(_inst);
         }
      }
      
      protected function registerObject(_id:int, type_nm:String, posX:Number, posY:Number, target:Dictionary, playerClass:int = -1, carry_item:Boolean = false) : void
      {
         var pos:int = 0;
         var _objects:Dictionary = target;
         var type:Class = this._icons[type_nm];
         if(!(type is Class))
         {
            throw new Error("Minimap::WrongObjectType: " + type_nm);
         }
         var _inst:MinimapObject = _objects[_id];
         if(Boolean(_inst))
         {
            pos = this._renderers.indexOf(_inst);
            if(pos != -1)
            {
               this._renderers.splice(pos,1);
            }
            delete _objects[_id];
            if(_inst is MinimapObjectBasix)
            {
               this.minimap.removeChild(_inst);
            }
            else
            {
               this.minimap._players.removeChild(_inst);
            }
         }
         _inst = new type();
         _inst.index = _id;
         _inst.updated = true;
         this._renderers.push(_inst);
         _inst.x = posX * this._cx - this._px;
         _inst.y = posY * this._cy - this._py;
         _inst.xx = posX;
         _inst.yy = posY;
         _objects[_id] = _inst;
         if(_inst is MinimapObjectPlayer)
         {
            (_inst as MinimapObjectPlayer).type = playerClass;
            (_inst as MinimapObjectPlayer).isCarryingItem = carry_item;
         }
         if(_inst is MinimapObjectBasix)
         {
            this.minimap.addChild(_inst);
         }
         else
         {
            this.minimap._players.addChild(_inst);
         }
      }
   }
}

